use std::collections::HashSet;

use axum::{
    extract::{Path, State},
    http::header,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::{
    auth::AuthUser,
    categories::{category_detail, CategoryDetail},
    error::AppError,
    meta::MetaClient,
    models::Category,
    prompts, AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/categories/{id}/generate", post(generate_category))
        .route("/assets/{id}/svg", get(asset_svg))
}

#[derive(Debug, Default, Deserialize)]
pub struct GenerateRequest {
    pub subcategory_count: Option<i64>,
    pub min_questions: Option<i64>,
    pub max_questions: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct PlanResponse {
    #[serde(default)]
    subcategories: Vec<PlanItem>,
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

/// Generates subcategories, questions and image specs for an owned category.
/// Runs synchronously and can take several minutes for 20 subcategories;
/// background jobs with live progress come later.
async fn generate_category(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    body: Option<Json<GenerateRequest>>,
) -> Result<Json<CategoryDetail>, AppError> {
    let client = MetaClient::from_config(&state.config).ok_or_else(|| {
        AppError::ServiceUnavailable(
            "META_API_KEY is not configured; generation is disabled".to_string(),
        )
    })?;

    let category = sqlx::query_as::<_, Category>(
        "SELECT id, owner_id, title, description, language, visibility, status, subcategory_count, created_at
         FROM categories WHERE id = ? AND owner_id = ?",
    )
    .bind(id)
    .bind(auth.0.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("category not found".to_string()))?;

    let params = body.map(|b| b.0).unwrap_or_default();
    let count = params
        .subcategory_count
        .unwrap_or(category.subcategory_count)
        .clamp(1, 50);
    let min_questions = params.min_questions.unwrap_or(5).clamp(1, 20);
    let max_questions = params.max_questions.unwrap_or(10).clamp(1, 20);
    if min_questions > max_questions {
        return Err(AppError::BadRequest(
            "min_questions must not exceed max_questions".to_string(),
        ));
    }

    sqlx::query("UPDATE categories SET status = 'generating', subcategory_count = ? WHERE id = ?")
        .bind(count)
        .bind(id)
        .execute(&state.db)
        .await?;

    let outcome = run_generation(
        &state.db,
        &client,
        &category,
        count,
        min_questions,
        max_questions,
    )
    .await;
    match outcome {
        Ok(()) => {
            sqlx::query("UPDATE categories SET status = 'ready' WHERE id = ?")
                .bind(id)
                .execute(&state.db)
                .await?;
            Ok(Json(category_detail(&state.db, auth.0.id, id).await?))
        }
        Err(err) => {
            sqlx::query("UPDATE categories SET status = 'failed' WHERE id = ?")
                .bind(id)
                .execute(&state.db)
                .await?;
            Err(err)
        }
    }
}

async fn run_generation(
    pool: &sqlx::SqlitePool,
    client: &MetaClient,
    category: &Category,
    count: i64,
    min_questions: i64,
    max_questions: i64,
) -> Result<(), AppError> {
    let subject = if category.description.trim().is_empty() {
        category.title.clone()
    } else {
        format!("{} ({})", category.title, category.description.trim())
    };

    let plan: PlanResponse = client
        .chat_json(
            &prompts::render_plan(&subject, &category.language, count),
            0.7,
        )
        .await?;
    if plan.subcategories.is_empty() {
        return Err(AppError::ServiceUnavailable(
            "generation backend returned no subcategories".to_string(),
        ));
    }

    // Regenerating replaces the previous content.
    sqlx::query("DELETE FROM subcategories WHERE category_id = ?")
        .bind(category.id)
        .execute(pool)
        .await?;

    for (index, item) in plan.subcategories.iter().take(count as usize).enumerate() {
        let title = item.title.trim();
        if title.is_empty() {
            continue;
        }
        let position = index as i64 + 1;
        let (subcategory_id,): (i64,) = sqlx::query_as(
            "INSERT INTO subcategories (category_id, position, title, summary)
             VALUES (?, ?, ?, ?) RETURNING id",
        )
        .bind(category.id)
        .bind(position)
        .bind(title)
        .bind(item.summary.trim())
        .fetch_one(pool)
        .await?;

        let subtopic = if item.summary.trim().is_empty() {
            title.to_string()
        } else {
            format!("{}: {}", title, item.summary.trim())
        };
        let lesson: LessonResponse = client
            .chat_json(
                &prompts::render_questions(
                    &subject,
                    &subtopic,
                    &category.language,
                    min_questions,
                    max_questions,
                ),
                0.7,
            )
            .await?;
        store_lesson(pool, subcategory_id, &lesson, max_questions as usize).await?;
    }
    Ok(())
}

async fn store_lesson(
    pool: &sqlx::SqlitePool,
    subcategory_id: i64,
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
            "INSERT INTO assets (subcategory_id, asset_key, spec_json) VALUES (?, ?, ?)
             ON CONFLICT (subcategory_id, asset_key) DO UPDATE SET spec_json = excluded.spec_json",
        )
        .bind(subcategory_id)
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
        position += 1;
        let options = serde_json::to_string(&question.options_asset_ids).unwrap_or_else(|_| "[]".to_string());
        sqlx::query(
            "INSERT INTO questions (subcategory_id, position, qid, text, qtype, options_json, correct_asset)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(subcategory_id)
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
        && !question.options_asset_ids.is_empty()
        && question
            .options_asset_ids
            .iter()
            .all(|id| asset_ids.contains(id.as_str()))
        && asset_ids.contains(question.correct_asset_id.as_str())
        && question.options_asset_ids.contains(&question.correct_asset_id)
}

/// Serves an asset's SVG, generating and caching it on first request.
async fn asset_svg(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT a.spec_json, a.svg_text FROM assets a
         JOIN subcategories s ON s.id = a.subcategory_id
         JOIN categories c ON c.id = s.category_id
         WHERE a.id = ? AND (c.visibility = 'public' OR c.owner_id = ?)",
    )
    .bind(id)
    .bind(auth.0.id)
    .fetch_optional(&state.db)
    .await?;
    let (spec_json, cached) = row.ok_or_else(|| AppError::NotFound("asset not found".to_string()))?;
    if let Some(svg) = cached {
        return Ok(svg_response(svg));
    }

    let client = MetaClient::from_config(&state.config).ok_or_else(|| {
        AppError::ServiceUnavailable(
            "META_API_KEY is not configured; image generation is disabled".to_string(),
        )
    })?;
    let svg = client.chat_text(&prompts::render_svg(&spec_json)).await?;
    let svg = svg.trim().to_string();
    if !svg.starts_with("<svg") {
        return Err(AppError::ServiceUnavailable(
            "generation backend returned invalid SVG".to_string(),
        ));
    }
    sqlx::query("UPDATE assets SET svg_text = ? WHERE id = ?")
        .bind(&svg)
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(svg_response(svg))
}

fn svg_response(svg: String) -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "image/svg+xml")], svg)
}
