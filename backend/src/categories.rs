use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{auth::AuthUser, error::AppError, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/categories", get(list_categories).post(create_category))
        .route(
            "/categories/{id}",
            get(get_category).delete(delete_category),
        )
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CategorySummary {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub language: String,
    pub visibility: String,
    pub status: String,
    pub subcategory_count: i64,
    pub owner_name: String,
    pub mine: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateCategory {
    pub title: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub visibility: Option<String>,
    pub subcategory_count: Option<i64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SubcategoryWithCount {
    pub id: i64,
    pub position: i64,
    pub title: String,
    pub summary: String,
    pub question_count: i64,
}

#[derive(Debug, Serialize)]
pub struct CategoryDetail {
    #[serde(flatten)]
    pub category: CategorySummary,
    pub subcategories: Vec<SubcategoryWithCount>,
}

const SUMMARY_COLUMNS: &str = "c.id, c.title, c.description, c.language, c.visibility, c.status,
    c.subcategory_count, u.name AS owner_name, (c.owner_id = ?1) AS mine, c.created_at
    FROM categories c JOIN users u ON u.id = c.owner_id";

async fn list_categories(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<CategorySummary>>, AppError> {
    let rows = sqlx::query_as::<_, CategorySummary>(&format!(
        "SELECT {SUMMARY_COLUMNS} WHERE c.visibility = 'public' OR c.owner_id = ?1 ORDER BY c.id DESC"
    ))
    .bind(auth.0.id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

async fn create_category(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateCategory>,
) -> Result<(StatusCode, Json<CategorySummary>), AppError> {
    let title = input.title.trim().to_string();
    if title.is_empty() || title.len() > 120 {
        return Err(AppError::BadRequest(
            "title must be 1-120 characters".to_string(),
        ));
    }
    let visibility = input.visibility.unwrap_or_else(|| "private".to_string());
    if visibility != "public" && visibility != "private" {
        return Err(AppError::BadRequest(
            "visibility must be 'public' or 'private'".to_string(),
        ));
    }
    let language = input.language.unwrap_or_else(|| "en".to_string());
    let language = language.trim().to_string();
    if language.is_empty() || language.len() > 8 {
        return Err(AppError::BadRequest("language must be 1-8 characters".to_string()));
    }
    let subcategory_count = input.subcategory_count.unwrap_or(20).clamp(1, 50);
    let description = input.description.unwrap_or_default();

    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO categories (owner_id, title, description, language, visibility, status, subcategory_count)
         VALUES (?, ?, ?, ?, ?, 'ready', ?) RETURNING id",
    )
    .bind(auth.0.id)
    .bind(&title)
    .bind(&description)
    .bind(&language)
    .bind(&visibility)
    .bind(subcategory_count)
    .fetch_one(&state.db)
    .await?;

    let summary = sqlx::query_as::<_, CategorySummary>(&format!(
        "SELECT {SUMMARY_COLUMNS} WHERE c.id = ?2"
    ))
    .bind(auth.0.id)
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    Ok((StatusCode::CREATED, Json(summary)))
}

async fn get_category(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<CategoryDetail>, AppError> {
    let category = sqlx::query_as::<_, CategorySummary>(&format!(
        "SELECT {SUMMARY_COLUMNS} WHERE c.id = ?2 AND (c.visibility = 'public' OR c.owner_id = ?1)"
    ))
    .bind(auth.0.id)
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("category not found".to_string()))?;

    let subcategories = sqlx::query_as::<_, SubcategoryWithCount>(
        "SELECT s.id, s.position, s.title, s.summary, COUNT(q.id) AS question_count
         FROM subcategories s LEFT JOIN questions q ON q.subcategory_id = s.id
         WHERE s.category_id = ? GROUP BY s.id ORDER BY s.position",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(CategoryDetail {
        category,
        subcategories,
    }))
}

async fn delete_category(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM categories WHERE id = ? AND owner_id = ?")
        .bind(id)
        .bind(auth.0.id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("category not found".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}
