use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::Semaphore;

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use tracing::Instrument;

use crate::{
    auth::AuthUser,
    error::AppError,
    meta::{CallContext, MetaClient},
    models::Subject,
    prompts,
    subjects::{subject_detail, SubjectDetail},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/subjects/{id}/generate", post(generate_subject))
        .route("/assets/{id}/svg", get(asset_svg))
}

#[derive(Debug, Default, Deserialize)]
pub struct GenerateRequest {
    pub lesson_count: Option<i64>,
    pub min_questions: Option<i64>,
    pub max_questions: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct PlanResponse {
    #[serde(default)]
    lessons: Vec<PlanItem>,
}

#[derive(Debug, Deserialize)]
struct PlanItem {
    #[serde(default)]
    title: String,
    #[serde(default)]
    summary: String,
}

#[derive(Debug, Deserialize)]
struct LessonResponse {
    #[serde(default)]
    questions: Vec<LessonQuestion>,
    #[serde(default)]
    assets: Vec<LessonAsset>,
}

#[derive(Debug, Deserialize)]
struct LessonQuestion {
    #[serde(default)]
    id: String,
    #[serde(default)]
    text: String,
    #[serde(rename = "type", default)]
    qtype: String,
    #[serde(default)]
    focus: String,
    #[serde(default)]
    options_asset_ids: Vec<String>,
    #[serde(default)]
    correct_asset_id: String,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct LessonAsset {
    #[serde(default)]
    id: String,
    #[serde(rename = "type", default)]
    atype: String,
    #[serde(default)]
    shape: Option<String>,
    #[serde(default)]
    description: String,
    #[serde(default)]
    main_color: String,
    #[serde(default)]
    secondary_color: String,
}

/// Starts generating lessons, questions and image specs for an owned
/// subject. Returns 202 immediately; the client polls the detail endpoint
/// until the status flips to `ready` or `failed`.
async fn generate_subject(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    body: Option<Json<GenerateRequest>>,
) -> Result<(StatusCode, Json<SubjectDetail>), AppError> {
    let Some(client) = MetaClient::from_config(&state.config)
        .map(|client| client.with_recorder(state.db.clone(), auth.0.id))
    else {
        tracing::warn!("generation requested but META_API_KEY is not set");
        return Err(AppError::ServiceUnavailable(
            "META_API_KEY is not configured; generation is disabled".to_string(),
        ));
    };

    let subject = sqlx::query_as::<_, Subject>(
        "SELECT id, owner_id, title, description, language, visibility, status, lesson_count, tags_json, created_at
         FROM subjects WHERE id = ? AND owner_id = ?",
    )
    .bind(id)
    .bind(auth.0.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("subject not found".to_string()))?;

    let params = body.map(|b| b.0).unwrap_or_default();
    let count = params
        .lesson_count
        .unwrap_or(subject.lesson_count)
        .clamp(1, 50);
    let min_questions = params.min_questions.unwrap_or(5).clamp(1, 20);
    let max_questions = params.max_questions.unwrap_or(10).clamp(1, 20);
    if min_questions > max_questions {
        return Err(AppError::BadRequest(
            "min_questions must not exceed max_questions".to_string(),
        ));
    }

    sqlx::query(
        "UPDATE subjects SET status = 'generating', lesson_count = ?, generated_lessons = 0 WHERE id = ?",
    )
    .bind(count)
    .bind(id)
    .execute(&state.db)
    .await?;

    tracing::info!(subject_id = id, lessons = count, "generation started");
    // The job runs in this process; a restart abandons it as `generating`.
    // A durable queue comes later.
    let pool = state.db.clone();
    let span = tracing::info_span!("generate", subject_id = subject.id);
    tokio::spawn(
        async move {
            match run_lessons(
                &pool,
                &client,
                &subject,
                count,
                min_questions,
                max_questions,
            )
            .await
            {
                Ok(()) => {
                    let _ = sqlx::query("UPDATE subjects SET status = 'ready' WHERE id = ?")
                        .bind(subject.id)
                        .execute(&pool)
                        .await;
                    tracing::info!("generation ready");
                    // Best effort: anything still missing renders on first view.
                    pregenerate_svgs(&pool, &client, subject.id).await;
                }
                Err(err) => {
                    let _ = sqlx::query("UPDATE subjects SET status = 'failed' WHERE id = ?")
                        .bind(subject.id)
                        .execute(&pool)
                        .await;
                    tracing::error!(error = ?err, "generation failed");
                }
            }
        }
        .instrument(span),
    );

    let detail = subject_detail(&state.db, auth.0.id, id).await?;
    Ok((StatusCode::ACCEPTED, Json(detail)))
}

/// How many lesson generations (and later SVG renders) run at once.
const FAN_OUT: usize = 5;

struct LessonJob {
    pool: sqlx::SqlitePool,
    client: MetaClient,
    subject_id: i64,
    topic: String,
    language: String,
    position: i64,
    item: PlanItem,
    min_questions: i64,
    max_questions: i64,
}

async fn run_lessons(
    pool: &sqlx::SqlitePool,
    client: &MetaClient,
    subject: &Subject,
    count: i64,
    min_questions: i64,
    max_questions: i64,
) -> Result<(), AppError> {
    let topic = if subject.description.trim().is_empty() {
        subject.title.clone()
    } else {
        format!("{} ({})", subject.title, subject.description.trim())
    };

    let ctx = CallContext {
        kind: "plan",
        subject_id: Some(subject.id),
        label: topic.chars().take(80).collect(),
        ..Default::default()
    };
    let plan: PlanResponse = client
        .chat_json(
            &ctx,
            &prompts::render_plan(&topic, &subject.language, count),
            0.7,
        )
        .await?;
    if plan.lessons.is_empty() {
        return Err(AppError::ServiceUnavailable(
            "generation backend returned no lessons".to_string(),
        ));
    }
    tracing::info!(lessons = plan.lessons.len(), "plan ready");

    // Regenerating replaces the previous content.
    sqlx::query("DELETE FROM lessons WHERE subject_id = ?")
        .bind(subject.id)
        .execute(pool)
        .await?;

    let items: Vec<(i64, PlanItem)> = plan
        .lessons
        .into_iter()
        .take(count as usize)
        .enumerate()
        .filter(|(_, item)| !item.title.trim().is_empty())
        .map(|(index, item)| (index as i64 + 1, item))
        .collect();

    let semaphore = Arc::new(Semaphore::new(FAN_OUT));
    let mut handles = Vec::with_capacity(items.len());
    for (position, item) in items {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| AppError::Internal("generation semaphore closed".to_string()))?;
        let job = LessonJob {
            pool: pool.clone(),
            client: client.clone(),
            subject_id: subject.id,
            topic: topic.clone(),
            language: subject.language.clone(),
            position,
            item,
            min_questions,
            max_questions,
        };
        let lesson_span = tracing::info_span!("lesson", position);
        handles.push(tokio::spawn(
            async move {
                let _permit = permit;
                generate_one_lesson(job).await
            }
            .instrument(lesson_span),
        ));
    }
    for handle in handles {
        handle
            .await
            .map_err(|err| AppError::Internal(format!("lesson task failed: {err}")))??;
    }
    Ok(())
}

async fn generate_one_lesson(job: LessonJob) -> Result<(), AppError> {
    let title = job.item.title.trim().to_string();
    let summary = job.item.summary.trim().to_string();
    let (lesson_id,): (i64,) = sqlx::query_as(
        "INSERT INTO lessons (subject_id, position, title, summary)
         VALUES (?, ?, ?, ?) RETURNING id",
    )
    .bind(job.subject_id)
    .bind(job.position)
    .bind(&title)
    .bind(&summary)
    .fetch_one(&job.pool)
    .await?;

    let lesson_topic = if summary.is_empty() {
        title.clone()
    } else {
        format!("{title}: {summary}")
    };
    let ctx = CallContext {
        kind: "questions",
        subject_id: Some(job.subject_id),
        lesson_id: Some(lesson_id),
        label: title.clone(),
        ..Default::default()
    };
    let lesson: LessonResponse = job
        .client
        .chat_json(
            &ctx,
            &prompts::render_questions(
                &job.topic,
                &lesson_topic,
                &job.language,
                job.min_questions,
                job.max_questions,
            ),
            0.7,
        )
        .await?;
    store_lesson(&job.pool, lesson_id, &lesson, job.max_questions as usize).await?;
    sqlx::query("UPDATE subjects SET generated_lessons = generated_lessons + 1 WHERE id = ?")
        .bind(job.subject_id)
        .execute(&job.pool)
        .await?;
    tracing::info!(title = %title, "lesson done");
    Ok(())
}

/// Renders every still-missing SVG so opening lessons is instant.
/// Best effort: failures are ignored, the lazy path covers them.
async fn pregenerate_svgs(pool: &sqlx::SqlitePool, client: &MetaClient, subject_id: i64) {
    let assets: Vec<(i64, String, i64, String)> = match sqlx::query_as(
        "SELECT a.id, a.asset_key, a.lesson_id, a.spec_json FROM assets a
         JOIN lessons s ON s.id = a.lesson_id
         WHERE s.subject_id = ? AND a.svg_text IS NULL",
    )
    .bind(subject_id)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(err) => {
            tracing::error!(%err, "svg prefill lookup failed");
            return;
        }
    };
    if assets.is_empty() {
        return;
    }
    tracing::info!(missing = assets.len(), "svg prefill started");
    let total = assets.len();
    let semaphore = Arc::new(Semaphore::new(FAN_OUT));
    let mut handles = Vec::with_capacity(total);
    for (id, key, lesson_id, spec) in assets {
        let Ok(permit) = semaphore.clone().acquire_owned().await else {
            break;
        };
        let pool = pool.clone();
        let client = client.clone();
        let ctx = CallContext {
            kind: "svg",
            subject_id: Some(subject_id),
            lesson_id: Some(lesson_id),
            asset_id: Some(id),
            label: key,
        };
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            render_and_store_svg(&pool, &client, &ctx, id, &spec).await
        }));
    }
    let mut failed = 0;
    for handle in handles {
        match handle.await {
            Ok(Ok(Some(_))) => {}
            _ => failed += 1,
        }
    }
    tracing::info!(rendered = total - failed, failed, "svg prefill done");
}

/// Renders one SVG and caches it. `Ok(None)` means the model answered
/// something that is not an SVG.
async fn render_and_store_svg(
    pool: &sqlx::SqlitePool,
    client: &MetaClient,
    ctx: &CallContext,
    asset_id: i64,
    spec_json: &str,
) -> Result<Option<String>, AppError> {
    let svg = client
        .chat_text(ctx, &prompts::render_svg(spec_json))
        .await?;
    let svg = svg.trim().to_string();
    if !svg.starts_with("<svg") {
        return Ok(None);
    }
    sqlx::query("UPDATE assets SET svg_text = ? WHERE id = ? AND svg_text IS NULL")
        .bind(&svg)
        .bind(asset_id)
        .execute(pool)
        .await?;
    Ok(Some(svg))
}

async fn store_lesson(
    pool: &sqlx::SqlitePool,
    lesson_id: i64,
    lesson: &LessonResponse,
    max_questions: usize,
) -> Result<(), AppError> {
    let mut asset_ids: HashSet<&str> = HashSet::new();
    for asset in &lesson.assets {
        if asset.id.trim().is_empty() {
            continue;
        }
        asset_ids.insert(asset.id.as_str());
        let spec = serde_json::to_string(asset).unwrap_or_else(|_| "{}".to_string());
        sqlx::query(
            "INSERT INTO assets (lesson_id, asset_key, spec_json) VALUES (?, ?, ?)
             ON CONFLICT (lesson_id, asset_key) DO UPDATE SET spec_json = excluded.spec_json",
        )
        .bind(lesson_id)
        .bind(asset.id.as_str())
        .bind(spec)
        .execute(pool)
        .await?;
    }

    let mut position = 0;
    for question in &lesson.questions {
        if position as usize >= max_questions {
            break;
        }
        if !valid_question(question, &asset_ids) {
            continue;
        }
        tracing::debug!(qid = %question.id, focus = %question.focus, "question focus");
        position += 1;
        let options =
            serde_json::to_string(&question.options_asset_ids).unwrap_or_else(|_| "[]".to_string());
        sqlx::query(
            "INSERT INTO questions (lesson_id, position, qid, text, qtype, options_json, correct_asset)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(lesson_id)
        .bind(position)
        .bind(if question.id.trim().is_empty() {
            format!("q{position}")
        } else {
            question.id.clone()
        })
        .bind(question.text.trim())
        .bind(question.qtype.as_str())
        .bind(options)
        .bind(question.correct_asset_id.as_str())
        .execute(pool)
        .await?;
    }
    Ok(())
}

fn valid_question(question: &LessonQuestion, asset_ids: &HashSet<&str>) -> bool {
    !question.text.trim().is_empty()
        && !question.focus.trim().is_empty()
        && !question.options_asset_ids.is_empty()
        && question
            .options_asset_ids
            .iter()
            .all(|id| asset_ids.contains(id.as_str()))
        && asset_ids.contains(question.correct_asset_id.as_str())
        && question
            .options_asset_ids
            .contains(&question.correct_asset_id)
}

/// Serves an asset's SVG, generating and caching it on first request.
async fn asset_svg(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let row: Option<(String, Option<String>, String, i64, i64)> = sqlx::query_as(
        "SELECT a.spec_json, a.svg_text, a.asset_key, s.id, c.id FROM assets a
         JOIN lessons s ON s.id = a.lesson_id
         JOIN subjects c ON c.id = s.subject_id
         WHERE a.id = ? AND (c.visibility = 'public' OR c.owner_id = ?)",
    )
    .bind(id)
    .bind(auth.0.id)
    .fetch_optional(&state.db)
    .await?;
    let (spec_json, cached, key, lesson_id, subject_id) =
        row.ok_or_else(|| AppError::NotFound("asset not found".to_string()))?;
    if let Some(svg) = cached {
        return Ok(svg_response(svg));
    }

    let Some(client) = MetaClient::from_config(&state.config)
        .map(|client| client.with_recorder(state.db.clone(), auth.0.id))
    else {
        tracing::warn!("image requested but META_API_KEY is not set");
        return Err(AppError::ServiceUnavailable(
            "META_API_KEY is not configured; image generation is disabled".to_string(),
        ));
    };
    let ctx = CallContext {
        kind: "svg",
        subject_id: Some(subject_id),
        lesson_id: Some(lesson_id),
        asset_id: Some(id),
        label: key,
    };
    let svg = render_and_store_svg(&state.db, &client, &ctx, id, &spec_json)
        .await?
        .ok_or_else(|| {
            tracing::warn!(asset_id = id, "generation backend returned invalid SVG");
            AppError::ServiceUnavailable("generation backend returned invalid SVG".to_string())
        })?;
    Ok(svg_response(svg))
}

fn svg_response(svg: String) -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "image/svg+xml")], svg)
}
