use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{auth::AuthUser, error::AppError, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/subjects", get(list_subjects).post(create_subject))
        .route(
            "/subjects/{id}",
            get(get_subject).delete(delete_subject),
        )
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SubjectSummary {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub language: String,
    pub visibility: String,
    pub status: String,
    pub lesson_count: i64,
    pub owner_name: String,
    pub mine: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSubject {
    pub title: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub visibility: Option<String>,
    pub lesson_count: Option<i64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LessonWithCount {
    pub id: i64,
    pub position: i64,
    pub title: String,
    pub summary: String,
    pub question_count: i64,
}

#[derive(Debug, Serialize)]
pub struct SubjectDetail {
    #[serde(flatten)]
    pub subject: SubjectSummary,
    pub lessons: Vec<LessonWithCount>,
}

const SUMMARY_COLUMNS: &str = "c.id, c.title, c.description, c.language, c.visibility, c.status,
    c.lesson_count, u.name AS owner_name, (c.owner_id = ?1) AS mine, c.created_at
    FROM subjects c JOIN users u ON u.id = c.owner_id";

#[derive(Debug, Default, Deserialize)]
struct ListQuery {
    q: Option<String>,
}

async fn list_subjects(
    auth: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<SubjectSummary>>, AppError> {
    let needle = query.q.unwrap_or_default().trim().to_string();
    let mut sql = format!(
        "SELECT {SUMMARY_COLUMNS} WHERE (c.visibility = 'public' OR c.owner_id = ?1)"
    );
    if !needle.is_empty() {
        sql.push_str(" AND (c.title LIKE ?2 ESCAPE '\\' OR c.description LIKE ?2 ESCAPE '\\')");
    }
    sql.push_str(" ORDER BY c.id DESC");
    let mut select = sqlx::query_as::<_, SubjectSummary>(&sql).bind(auth.0.id);
    if !needle.is_empty() {
        select = select.bind(format!("%{}%", escape_like(&needle)));
    }
    Ok(Json(select.fetch_all(&state.db).await?))
}

fn escape_like(needle: &str) -> String {
    needle.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

async fn create_subject(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(input): Json<CreateSubject>,
) -> Result<(StatusCode, Json<SubjectSummary>), AppError> {
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
    let lesson_count = input.lesson_count.unwrap_or(20).clamp(1, 50);
    let description = input.description.unwrap_or_default();

    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO subjects (owner_id, title, description, language, visibility, status, lesson_count)
         VALUES (?, ?, ?, ?, ?, 'ready', ?) RETURNING id",
    )
    .bind(auth.0.id)
    .bind(&title)
    .bind(&description)
    .bind(&language)
    .bind(&visibility)
    .bind(lesson_count)
    .fetch_one(&state.db)
    .await?;

    let summary = sqlx::query_as::<_, SubjectSummary>(&format!(
        "SELECT {SUMMARY_COLUMNS} WHERE c.id = ?2"
    ))
    .bind(auth.0.id)
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    Ok((StatusCode::CREATED, Json(summary)))
}

async fn get_subject(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<SubjectDetail>, AppError> {
    Ok(Json(subject_detail(&state.db, auth.0.id, id).await?))
}

/// Loads a subject with its ordered lessons, enforcing visibility.
/// Shared by the detail endpoint and the generator.
pub async fn subject_detail(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    subject_id: i64,
) -> Result<SubjectDetail, AppError> {
    let subject = sqlx::query_as::<_, SubjectSummary>(&format!(
        "SELECT {SUMMARY_COLUMNS} WHERE c.id = ?2 AND (c.visibility = 'public' OR c.owner_id = ?1)"
    ))
    .bind(user_id)
    .bind(subject_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("subject not found".to_string()))?;

    let lessons = sqlx::query_as::<_, LessonWithCount>(
        "SELECT s.id, s.position, s.title, s.summary, COUNT(q.id) AS question_count
         FROM lessons s LEFT JOIN questions q ON q.lesson_id = s.id
         WHERE s.subject_id = ? GROUP BY s.id ORDER BY s.position",
    )
    .bind(subject_id)
    .fetch_all(pool)
    .await?;

    Ok(SubjectDetail {
        subject,
        lessons,
    })
}

async fn delete_subject(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM subjects WHERE id = ? AND owner_id = ?")
        .bind(id)
        .bind(auth.0.id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("subject not found".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}
