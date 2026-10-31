use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
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
        .route("/subjects/{id}/like", post(like_subject).delete(unlike_subject))
}

#[derive(Debug, sqlx::FromRow)]
struct SubjectRow {
    id: i64,
    title: String,
    description: String,
    language: String,
    visibility: String,
    status: String,
    lesson_count: i64,
    owner_name: String,
    mine: bool,
    like_count: i64,
    liked_by_me: bool,
    tags_json: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
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
    pub like_count: i64,
    pub liked_by_me: bool,
    pub tags: Vec<String>,
    pub created_at: String,
}

impl From<SubjectRow> for SubjectSummary {
    fn from(row: SubjectRow) -> Self {
        let tags = serde_json::from_str(&row.tags_json).unwrap_or_default();
        Self {
            id: row.id,
            title: row.title,
            description: row.description,
            language: row.language,
            visibility: row.visibility,
            status: row.status,
            lesson_count: row.lesson_count,
            owner_name: row.owner_name,
            mine: row.mine,
            like_count: row.like_count,
            liked_by_me: row.liked_by_me,
            tags,
            created_at: row.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateSubject {
    pub title: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub visibility: Option<String>,
    pub lesson_count: Option<i64>,
    pub tags: Option<Vec<String>>,
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
    c.lesson_count, u.name AS owner_name, (c.owner_id = ?1) AS mine,
    (SELECT COUNT(*) FROM likes l WHERE l.subject_id = c.id) AS like_count,
    EXISTS(SELECT 1 FROM likes l WHERE l.subject_id = c.id AND l.user_id = ?1) AS liked_by_me,
    c.tags_json,
    c.created_at
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
        sql.push_str(
            " AND (c.title LIKE ?2 ESCAPE '\\' OR c.description LIKE ?2 ESCAPE '\\'
             OR c.tags_json LIKE ?2 ESCAPE '\\')",
        );
    }
    sql.push_str(" ORDER BY c.id DESC");
    let mut select = sqlx::query_as::<_, SubjectRow>(&sql).bind(auth.0.id);
    if !needle.is_empty() {
        select = select.bind(format!("%{}%", escape_like(&needle)));
    }
    let rows: Vec<SubjectRow> = select.fetch_all(&state.db).await?;
    Ok(Json(
        rows.into_iter().map(SubjectSummary::from).collect(),
    ))
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
    let tags = normalize_tags(input.tags.unwrap_or_default())?;
    let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());

    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO subjects (owner_id, title, description, language, visibility, status, lesson_count, tags_json)
         VALUES (?, ?, ?, ?, ?, 'ready', ?, ?) RETURNING id",
    )
    .bind(auth.0.id)
    .bind(&title)
    .bind(&description)
    .bind(&language)
    .bind(&visibility)
    .bind(lesson_count)
    .bind(&tags_json)
    .fetch_one(&state.db)
    .await?;

    let row = sqlx::query_as::<_, SubjectRow>(&format!(
        "SELECT {SUMMARY_COLUMNS} WHERE c.id = ?2"
    ))
    .bind(auth.0.id)
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    Ok((StatusCode::CREATED, Json(SubjectSummary::from(row))))
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
    let row = sqlx::query_as::<_, SubjectRow>(&format!(
        "SELECT {SUMMARY_COLUMNS} WHERE c.id = ?2 AND (c.visibility = 'public' OR c.owner_id = ?1)"
    ))
    .bind(user_id)
    .bind(subject_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("subject not found".to_string()))?;
    let subject = SubjectSummary::from(row);

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

async fn subject_summary(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    subject_id: i64,
) -> Result<SubjectSummary, AppError> {
    let row = sqlx::query_as::<_, SubjectRow>(&format!(
        "SELECT {SUMMARY_COLUMNS} WHERE c.id = ?2 AND (c.visibility = 'public' OR c.owner_id = ?1)"
    ))
    .bind(user_id)
    .bind(subject_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("subject not found".to_string()))?;
    Ok(SubjectSummary::from(row))
}

/// Lowercases, dedupes and validates tags: at most 8, each 1-24 chars.
fn normalize_tags(tags: Vec<String>) -> Result<Vec<String>, AppError> {
    if tags.len() > 8 {
        return Err(AppError::BadRequest("at most 8 tags allowed".to_string()));
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for tag in tags {
        let tag = tag.trim().to_lowercase();
        if tag.is_empty() || tag.len() > 24 {
            return Err(AppError::BadRequest(
                "each tag must be 1-24 characters".to_string(),
            ));
        }
        if seen.insert(tag.clone()) {
            out.push(tag);
        }
    }
    Ok(out)
}

/// Likes a visible subject. Idempotent; liking your own is rejected.
async fn like_subject(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<SubjectSummary>, AppError> {
    let summary = subject_summary(&state.db, auth.0.id, id).await?;
    if summary.mine {
        return Err(AppError::BadRequest(
            "cannot like your own subject".to_string(),
        ));
    }
    sqlx::query("INSERT OR IGNORE INTO likes (user_id, subject_id) VALUES (?, ?)")
        .bind(auth.0.id)
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(Json(subject_summary(&state.db, auth.0.id, id).await?))
}

async fn unlike_subject(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<SubjectSummary>, AppError> {
    subject_summary(&state.db, auth.0.id, id).await?;
    sqlx::query("DELETE FROM likes WHERE user_id = ? AND subject_id = ?")
        .bind(auth.0.id)
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(Json(subject_summary(&state.db, auth.0.id, id).await?))
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
