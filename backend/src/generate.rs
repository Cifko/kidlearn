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
    models::{Lesson, PlotSpec, Subject},
    prompts,
    subjects::{subject_detail, SubjectDetail},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/subjects/{id}/generate", post(generate_subject))
        .route("/assets/{id}/image", get(asset_image))
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
    #[serde(default)]
    options_text: Vec<String>,
    #[serde(default)]
    correct_text: String,
    #[serde(default)]
    figure_asset_id: String,
    #[serde(default)]
    plot: Option<PlotSpec>,
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
        "SELECT id, owner_id, title, description, language, visibility, status, lesson_count, tags_json, grade, modality, created_at
         FROM subjects WHERE id = ? AND owner_id = ?",
    )
    .bind(id)
    .bind(auth.0.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("subject not found".to_string()))?;
    if subject.visibility == "public" {
        return Err(AppError::BadRequest(
            "cannot regenerate a published subject".to_string(),
        ));
    }

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
                    pregenerate_images(&pool, &client, subject.id).await;
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

/// How many lesson generations (and later image renders) run at once.
const FAN_OUT: usize = 5;

struct LessonJob {
    pool: sqlx::SqlitePool,
    client: MetaClient,
    subject_id: i64,
    topic: String,
    language: String,
    grade: Option<i64>,
    modality: String,
    prerequisites: String,
    position: i64,
    item: PlanItem,
    min_questions: i64,
    max_questions: i64,
}

/// Prompt budget for prerequisite context. Lesson summaries already name
/// concrete skills, so titles + summaries map the prior coverage without
/// pasting every question.
const PREREQ_BUDGET: usize = 6000;
/// How many link hops back the context reaches.
const PREREQ_DEPTH: usize = 4;

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

/// Transitive prerequisite context for the prompts: titles, descriptions
/// and lesson plans of every ancestor subject, nearest first. Only
/// subjects visible to the generator (own or public) are included.
/// "None" when there is nothing to build on.
async fn prerequisite_context(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    subject_id: i64,
) -> Result<String, AppError> {
    let links: Vec<(i64, i64)> =
        sqlx::query_as("SELECT subject_id, prerequisite_id FROM subject_prerequisites")
            .fetch_all(pool)
            .await?;
    let mut children: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    for (from, to) in links {
        children.entry(from).or_default().push(to);
    }
    let mut visited = HashSet::new();
    visited.insert(subject_id);
    let mut order = Vec::new();
    let mut frontier = vec![subject_id];
    for _ in 0..PREREQ_DEPTH {
        let mut next = Vec::new();
        for id in frontier {
            if let Some(prereqs) = children.get(&id) {
                for prereq in prereqs {
                    if visited.insert(*prereq) {
                        order.push(*prereq);
                        next.push(*prereq);
                    }
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    if order.is_empty() {
        return Ok("None".to_string());
    }
    let placeholders = order.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let subject_sql = format!(
        "SELECT id, owner_id, title, description, language, visibility, status, lesson_count, tags_json, grade, modality, created_at
         FROM subjects WHERE id IN ({placeholders}) AND (visibility = 'public' OR owner_id = ?)"
    );
    let mut select = sqlx::query_as::<_, Subject>(&subject_sql);
    for id in &order {
        select = select.bind(id);
    }
    let subjects: Vec<Subject> = select.bind(user_id).fetch_all(pool).await?;
    let by_id: std::collections::HashMap<i64, &Subject> = subjects
        .iter()
        .map(|subject| (subject.id, subject))
        .collect();
    let lesson_sql = format!(
        "SELECT id, subject_id, position, title, summary FROM lessons
         WHERE subject_id IN ({placeholders}) ORDER BY subject_id, position"
    );
    let mut lesson_select = sqlx::query_as::<_, Lesson>(&lesson_sql);
    for id in &order {
        lesson_select = lesson_select.bind(id);
    }
    let lessons: Vec<Lesson> = lesson_select.fetch_all(pool).await?;
    let mut lessons_by_subject: std::collections::HashMap<i64, Vec<&Lesson>> =
        std::collections::HashMap::new();
    for lesson in &lessons {
        lessons_by_subject
            .entry(lesson.subject_id)
            .or_default()
            .push(lesson);
    }
    let mut blocks = Vec::new();
    let mut used = 0;
    for id in order {
        let Some(subject) = by_id.get(&id) else {
            continue;
        };
        let mut block = format!(
            "Prior subject \"{}\": {}",
            subject.title,
            clip(subject.description.trim(), 300)
        );
        if let Some(lessons) = lessons_by_subject.get(&id) {
            for lesson in lessons {
                block.push_str(&format!(
                    "\n  Lesson {}: {} — {}",
                    lesson.position,
                    clip(lesson.title.trim(), 120),
                    clip(lesson.summary.trim(), 200)
                ));
            }
        }
        if used + block.len() > PREREQ_BUDGET {
            break;
        }
        used += block.len();
        blocks.push(block);
    }
    if blocks.is_empty() {
        return Ok("None".to_string());
    }
    Ok(blocks.join("\n\n"))
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
    let prerequisites = prerequisite_context(pool, subject.owner_id, subject.id).await?;
    if prerequisites != "None" {
        tracing::info!(chars = prerequisites.len(), "prerequisite context attached");
    }

    let ctx = CallContext {
        kind: "plan",
        subject_id: Some(subject.id),
        label: topic.chars().take(80).collect(),
        ..Default::default()
    };
    let plan: PlanResponse = client
        .chat_json(
            &ctx,
            &prompts::render_plan(
                &topic,
                &subject.language,
                count,
                subject.grade,
                &prerequisites,
            ),
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
            grade: subject.grade,
            modality: subject.modality.clone(),
            prerequisites: prerequisites.clone(),
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
                job.grade,
                &job.modality,
                &job.prerequisites,
            ),
            0.7,
        )
        .await?;
    store_lesson(
        &job.pool,
        lesson_id,
        &lesson,
        job.max_questions as usize,
        job.modality.as_str() == "text_only",
    )
    .await?;
    sqlx::query("UPDATE subjects SET generated_lessons = generated_lessons + 1 WHERE id = ?")
        .bind(job.subject_id)
        .execute(&job.pool)
        .await?;
    tracing::info!(title = %title, "lesson done");
    Ok(())
}

/// Renders every still-missing image so opening lessons is instant.
/// Best effort: failures are ignored, the lazy path covers them.
async fn pregenerate_images(pool: &sqlx::SqlitePool, client: &MetaClient, subject_id: i64) {
    let assets: Vec<(i64, String, i64, String)> = match sqlx::query_as(
        "SELECT a.id, a.asset_key, a.lesson_id, a.spec_json FROM assets a
         JOIN lessons s ON s.id = a.lesson_id
         WHERE s.subject_id = ? AND a.image_blob IS NULL",
    )
    .bind(subject_id)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(err) => {
            tracing::error!(%err, "image prefill lookup failed");
            return;
        }
    };
    if assets.is_empty() {
        return;
    }
    tracing::info!(missing = assets.len(), "image prefill started");
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
            kind: "image",
            subject_id: Some(subject_id),
            lesson_id: Some(lesson_id),
            asset_id: Some(id),
            label: key,
        };
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            render_and_store_image(&pool, &client, &ctx, id, &spec).await
        }));
    }
    let mut failed = 0;
    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => {}
            _ => failed += 1,
        }
    }
    tracing::info!(rendered = total - failed, failed, "image prefill done");
}

/// Renders one image and caches its bytes.
async fn render_and_store_image(
    pool: &sqlx::SqlitePool,
    client: &MetaClient,
    ctx: &CallContext,
    asset_id: i64,
    spec_json: &str,
) -> Result<(Vec<u8>, String), AppError> {
    let (bytes, mime) = client
        .generate_image(ctx, &prompts::render_image(spec_json))
        .await?;
    sqlx::query(
        "UPDATE assets SET image_blob = ?, image_mime = ? WHERE id = ? AND image_blob IS NULL",
    )
    .bind(&bytes)
    .bind(&mime)
    .bind(asset_id)
    .execute(pool)
    .await?;
    Ok((bytes, mime))
}

async fn store_lesson(
    pool: &sqlx::SqlitePool,
    lesson_id: i64,
    lesson: &LessonResponse,
    max_questions: usize,
    text_only: bool,
) -> Result<(), AppError> {
    let mut asset_ids: HashSet<&str> = HashSet::new();
    // Text-only lessons keep no picture specs: nothing can reference them,
    // and the image prefill finds nothing to render.
    if !text_only {
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
    }

    let mut position = 0;
    for question in &lesson.questions {
        if position as usize >= max_questions {
            break;
        }
        if !valid_question(question, &asset_ids, text_only) {
            continue;
        }
        tracing::debug!(qid = %question.id, focus = %question.focus, "question focus");
        position += 1;
        let options =
            serde_json::to_string(&question.options_asset_ids).unwrap_or_else(|_| "[]".to_string());
        let (text_options, text_correct) = cleaned_text_options(question);
        let options_text =
            serde_json::to_string(&text_options).unwrap_or_else(|_| "[]".to_string());
        let plot_json = question
            .plot
            .as_ref()
            .map(|plot| serde_json::to_string(plot).unwrap_or_default())
            .unwrap_or_default();
        sqlx::query(
            "INSERT INTO questions (lesson_id, position, qid, text, qtype, options_json, correct_asset,
                 options_text_json, correct_text, figure_asset, plot_json)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
        .bind(options_text)
        .bind(text_correct)
        .bind(question.figure_asset_id.trim())
        .bind(plot_json)
        .execute(pool)
        .await?;
    }
    Ok(())
}

fn valid_question(question: &LessonQuestion, asset_ids: &HashSet<&str>, text_only: bool) -> bool {
    if question.text.trim().is_empty() || question.focus.trim().is_empty() {
        return false;
    }
    // The backstop behind the prompt's text-only rule: a text-only lesson
    // can never serve a picture, even if the model slips one in.
    if text_only
        && (question.qtype.as_str() != "multiple_choice_text"
            || !question.figure_asset_id.trim().is_empty())
    {
        return false;
    }
    // A figure must point at a defined asset; empty means no figure.
    if !question.figure_asset_id.trim().is_empty()
        && !asset_ids.contains(question.figure_asset_id.trim())
    {
        return false;
    }
    // Plots are allowed in every modality: they are deterministic diagrams,
    // not AI pictures. Only the shape is checked; the app parses expressions.
    if let Some(plot) = &question.plot {
        if !valid_plot(plot) {
            return false;
        }
    }
    match question.qtype.as_str() {
        "multiple_choice_image" => {
            !question.options_asset_ids.is_empty()
                && question
                    .options_asset_ids
                    .iter()
                    .all(|id| asset_ids.contains(id.as_str()))
                && asset_ids.contains(question.correct_asset_id.as_str())
                && question
                    .options_asset_ids
                    .contains(&question.correct_asset_id)
        }
        "multiple_choice_text" => {
            let (options, correct) = cleaned_text_options(question);
            valid_text_options(&options, &correct)
        }
        _ => false,
    }
}

/// Trims text options for storage; empties are dropped.
fn cleaned_text_options(question: &LessonQuestion) -> (Vec<String>, String) {
    let options = question
        .options_text
        .iter()
        .map(|option| option.trim().to_string())
        .filter(|option| !option.is_empty())
        .collect();
    (options, question.correct_text.trim().to_string())
}

fn valid_plot(plot: &PlotSpec) -> bool {
    (1..=4).contains(&plot.functions.len())
        && plot
            .functions
            .iter()
            .all(|f| !f.expr.trim().is_empty() && f.expr.len() <= 200 && f.label.len() <= 80)
        && plot.points.len() <= 8
        && plot.points.iter().all(|p| p.label.len() <= 40)
        && plot.x_min.is_finite()
        && plot.x_max.is_finite()
        && plot.y_min.is_finite()
        && plot.y_max.is_finite()
        && plot.x_min < plot.x_max
        && plot.y_min < plot.y_max
}

/// Text options must offer a real choice: at least two distinct answers,
/// one of them the correct one.
fn valid_text_options(options: &[String], correct: &str) -> bool {
    options.len() >= 2
        && !correct.is_empty()
        && options.iter().any(|option| option == correct)
        && options.iter().collect::<HashSet<_>>().len() == options.len()
}

/// One row of the asset lookup: spec, cached bytes, cached MIME,
/// asset key, lesson id, subject id.
type AssetRow = (String, Option<Vec<u8>>, Option<String>, String, i64, i64);

/// Serves an asset's image, generating and caching it on first request.
async fn asset_image(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let row: Option<AssetRow> = sqlx::query_as(
        "SELECT a.spec_json, a.image_blob, a.image_mime, a.asset_key, s.id, c.id FROM assets a
             JOIN lessons s ON s.id = a.lesson_id
             JOIN subjects c ON c.id = s.subject_id
             WHERE a.id = ? AND (c.visibility = 'public' OR c.owner_id = ?)",
    )
    .bind(id)
    .bind(auth.0.id)
    .fetch_optional(&state.db)
    .await?;
    let (spec_json, cached_blob, cached_mime, key, lesson_id, subject_id) =
        row.ok_or_else(|| AppError::NotFound("asset not found".to_string()))?;
    if let (Some(blob), Some(mime)) = (cached_blob, cached_mime) {
        return Ok(image_response(blob, mime));
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
        kind: "image",
        subject_id: Some(subject_id),
        lesson_id: Some(lesson_id),
        asset_id: Some(id),
        label: key,
    };
    let (bytes, mime) = render_and_store_image(&state.db, &client, &ctx, id, &spec_json).await?;
    Ok(image_response(bytes, mime))
}

fn image_response(bytes: Vec<u8>, mime: String) -> impl IntoResponse {
    let mut headers = header::HeaderMap::new();
    let value = mime
        .parse()
        .unwrap_or_else(|_| header::HeaderValue::from_static("application/octet-stream"));
    headers.insert(header::CONTENT_TYPE, value);
    (headers, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assets() -> HashSet<&'static str> {
        HashSet::from(["a", "b"])
    }

    fn image_question() -> LessonQuestion {
        LessonQuestion {
            id: "q1".to_string(),
            text: "Which shows 2?".to_string(),
            qtype: "multiple_choice_image".to_string(),
            focus: "count to 2".to_string(),
            options_asset_ids: vec!["a".to_string(), "b".to_string()],
            correct_asset_id: "a".to_string(),
            options_text: Vec::new(),
            correct_text: String::new(),
            figure_asset_id: String::new(),
            plot: None,
        }
    }

    fn text_question() -> LessonQuestion {
        LessonQuestion {
            id: "q1".to_string(),
            text: "What is 1 + 1?".to_string(),
            qtype: "multiple_choice_text".to_string(),
            focus: "add to 2".to_string(),
            options_asset_ids: Vec::new(),
            correct_asset_id: String::new(),
            options_text: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            correct_text: "2".to_string(),
            figure_asset_id: String::new(),
            plot: None,
        }
    }

    fn plot() -> PlotSpec {
        PlotSpec {
            functions: vec![crate::models::PlotFunction {
                expr: "x^2".to_string(),
                label: "f(x) = x^2".to_string(),
            }],
            x_min: -5.0,
            x_max: 5.0,
            y_min: -2.0,
            y_max: 10.0,
            points: vec![crate::models::PlotPoint {
                x: 2.0,
                y: 4.0,
                label: String::new(),
            }],
        }
    }

    #[test]
    fn valid_questions_pass_per_type() {
        assert!(valid_question(&image_question(), &assets(), false));
        assert!(valid_question(&text_question(), &assets(), false));
    }

    #[test]
    fn text_only_rejects_image_questions() {
        assert!(!valid_question(&image_question(), &assets(), true));
        assert!(valid_question(&text_question(), &assets(), true));
    }

    #[test]
    fn figure_must_reference_a_known_asset() {
        let mut question = text_question();
        question.figure_asset_id = "a".to_string();
        assert!(valid_question(&question, &assets(), false));

        question.figure_asset_id = "missing".to_string();
        assert!(!valid_question(&question, &assets(), false));
    }

    #[test]
    fn text_only_rejects_figures() {
        let mut question = text_question();
        question.figure_asset_id = "a".to_string();
        assert!(!valid_question(&question, &assets(), true));
    }

    #[test]
    fn valid_plot_passes_and_bad_plot_fails() {
        let mut question = text_question();
        question.plot = Some(plot());
        assert!(valid_question(&question, &assets(), false));

        // Plots are deterministic diagrams, allowed even in text-only mode.
        assert!(valid_question(&question, &assets(), true));

        let mut bad = plot();
        bad.functions.clear();
        let mut question = text_question();
        question.plot = Some(bad);
        assert!(!valid_question(&question, &assets(), false));

        let mut bad = plot();
        bad.x_min = 5.0;
        bad.x_max = -5.0;
        let mut question = text_question();
        question.plot = Some(bad);
        assert!(!valid_question(&question, &assets(), false));

        let mut bad = plot();
        bad.functions[0].expr = String::new();
        let mut question = text_question();
        question.plot = Some(bad);
        assert!(!valid_question(&question, &assets(), false));
    }

    #[test]
    fn unknown_question_type_is_rejected() {
        let mut question = text_question();
        question.qtype = "multiple_choice".to_string();
        assert!(!valid_question(&question, &assets(), false));
    }

    #[test]
    fn image_questions_need_known_assets_and_focus() {
        let mut question = image_question();
        question.focus = String::new();
        assert!(!valid_question(&question, &assets(), false));

        let mut question = image_question();
        question.correct_asset_id = "missing".to_string();
        assert!(!valid_question(&question, &assets(), false));

        let mut question = image_question();
        question.options_asset_ids = vec!["a".to_string()];
        question.correct_asset_id = "b".to_string();
        assert!(!valid_question(&question, &assets(), false));
    }

    #[test]
    fn text_questions_need_distinct_options_with_correct() {
        let mut question = text_question();
        question.options_text = vec!["2".to_string()];
        assert!(!valid_question(&question, &assets(), false));

        let mut question = text_question();
        question.options_text = vec!["2".to_string(), "2".to_string()];
        assert!(!valid_question(&question, &assets(), false));

        let mut question = text_question();
        question.correct_text = "9".to_string();
        assert!(!valid_question(&question, &assets(), false));

        let mut question = text_question();
        question.options_text = vec!["2".to_string(), "  ".to_string()];
        assert!(!valid_question(&question, &assets(), false));
    }
}
