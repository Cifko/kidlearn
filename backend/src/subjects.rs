use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthUser,
    error::AppError,
    meta::{CallContext, MetaClient},
    prompts, AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/subjects", get(list_subjects).post(create_subject))
        .route("/subjects/{id}", get(get_subject).delete(delete_subject))
        .route(
            "/subjects/{id}/like",
            post(like_subject).delete(unlike_subject),
        )
        .route("/subjects/{id}/publish", post(publish_subject))
        .route("/subjects/ai-search", get(ai_search))
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
    generated_lessons: i64,
    grade: Option<i64>,
    modality: String,
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
    pub generated_lessons: i64,
    pub grade: Option<i64>,
    pub modality: String,
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
            generated_lessons: row.generated_lessons,
            grade: row.grade,
            modality: row.modality,
            created_at: row.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateSubject {
    pub title: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub lesson_count: Option<i64>,
    pub tags: Option<Vec<String>>,
    pub grade: Option<i64>,
    pub modality: Option<String>,
    pub prerequisite_ids: Option<Vec<i64>>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LessonWithCount {
    pub id: i64,
    pub position: i64,
    pub title: String,
    pub summary: String,
    pub question_count: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PrerequisiteRef {
    pub id: i64,
    pub title: String,
    pub owner_name: String,
}

#[derive(Debug, Serialize)]
pub struct SubjectDetail {
    #[serde(flatten)]
    pub subject: SubjectSummary,
    pub lessons: Vec<LessonWithCount>,
    pub prerequisites: Vec<PrerequisiteRef>,
}

const SUMMARY_COLUMNS: &str = "c.id, c.title, c.description, c.language, c.visibility, c.status,
    c.lesson_count, u.name AS owner_name, (c.owner_id = ?1) AS mine,
    (SELECT COUNT(*) FROM likes l WHERE l.subject_id = c.id) AS like_count,
    EXISTS(SELECT 1 FROM likes l WHERE l.subject_id = c.id AND l.user_id = ?1) AS liked_by_me,
    c.tags_json,
    c.generated_lessons,
    c.grade,
    c.modality,
    c.created_at
    FROM subjects c JOIN users u ON u.id = c.owner_id";

#[derive(Debug, Default, Deserialize)]
struct ListQuery {
    q: Option<String>,
    lang: Option<String>,
    visibility: Option<String>,
}

async fn list_subjects(
    auth: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<SubjectSummary>>, AppError> {
    let needle = query.q.unwrap_or_default().trim().to_string();
    let lang = query.lang.unwrap_or_default().trim().to_string();
    let visibility = query.visibility.unwrap_or_default().trim().to_string();
    let mut sql =
        format!("SELECT {SUMMARY_COLUMNS} WHERE (c.visibility = 'public' OR c.owner_id = ?1)");
    let mut param = 2;
    if !needle.is_empty() {
        sql.push_str(&format!(
            " AND (c.title LIKE ?{param} ESCAPE '\\' OR c.description LIKE ?{param} ESCAPE '\\'
             OR c.tags_json LIKE ?{param} ESCAPE '\\')"
        ));
        param += 1;
    }
    if !lang.is_empty() {
        sql.push_str(&format!(" AND c.language = ?{param}"));
        param += 1;
    }
    if visibility == "public" || visibility == "private" {
        sql.push_str(&format!(" AND c.visibility = ?{param}"));
    }
    sql.push_str(" ORDER BY c.id DESC");
    let mut select = sqlx::query_as::<_, SubjectRow>(&sql).bind(auth.0.id);
    if !needle.is_empty() {
        select = select.bind(format!("%{}%", escape_like(&needle)));
    }
    if !lang.is_empty() {
        select = select.bind(lang);
    }
    if visibility == "public" || visibility == "private" {
        select = select.bind(visibility);
    }
    let rows: Vec<SubjectRow> = select.fetch_all(&state.db).await?;
    Ok(Json(rows.into_iter().map(SubjectSummary::from).collect()))
}

fn escape_like(needle: &str) -> String {
    needle
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[derive(Debug, Default, Deserialize)]
struct AiSearchQuery {
    q: Option<String>,
    max_results: Option<i64>,
    lang: Option<String>,
}

#[derive(Debug, Serialize)]
struct SearchCandidate {
    id: i64,
    title: String,
    description: String,
    tags: Vec<String>,
    language: String,
}

#[derive(Debug, Deserialize)]
struct RankedIds {
    #[serde(default)]
    ids: Vec<i64>,
}

/// Asks the model to rank visible subjects by relevance to the query.
/// Only ids the model returns (that exist) come back, best first.
async fn ai_search(
    auth: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<AiSearchQuery>,
) -> Result<Json<Vec<SubjectSummary>>, AppError> {
    let needle = query.q.unwrap_or_default().trim().to_string();
    if needle.is_empty() {
        return Err(AppError::BadRequest("q is required".to_string()));
    }
    let client = MetaClient::from_config(&state.config)
        .map(|client| client.with_recorder(state.db.clone(), auth.0.id))
        .ok_or_else(|| {
            AppError::ServiceUnavailable(
                "META_API_KEY is not configured; AI search is disabled".to_string(),
            )
        })?;

    let lang = query.lang.unwrap_or_default().trim().to_string();
    let mut sql =
        format!("SELECT {SUMMARY_COLUMNS} WHERE (c.visibility = 'public' OR c.owner_id = ?1)");
    if !lang.is_empty() {
        sql.push_str(" AND c.language = ?2");
    }
    sql.push_str(" ORDER BY c.id DESC LIMIT 50");
    let mut select = sqlx::query_as::<_, SubjectRow>(&sql).bind(auth.0.id);
    if !lang.is_empty() {
        select = select.bind(lang);
    }
    let rows: Vec<SubjectRow> = select.fetch_all(&state.db).await?;
    if rows.is_empty() {
        return Ok(Json(Vec::new()));
    }

    let candidates: Vec<SearchCandidate> = rows
        .iter()
        .map(|row| SearchCandidate {
            id: row.id,
            title: row.title.clone(),
            description: row.description.chars().take(200).collect(),
            tags: serde_json::from_str(&row.tags_json).unwrap_or_default(),
            language: row.language.clone(),
        })
        .collect();
    let max_results = query.max_results.unwrap_or(10).clamp(1, 20);
    let ctx = CallContext {
        kind: "search",
        label: needle.chars().take(80).collect(),
        ..Default::default()
    };
    let ranked: RankedIds = client
        .chat_json(
            &ctx,
            &prompts::render_search(
                &needle,
                &serde_json::to_string(&candidates).unwrap_or_else(|_| "[]".to_string()),
                max_results,
            ),
            0.2,
        )
        .await?;

    let order: std::collections::HashMap<i64, usize> = ranked
        .ids
        .iter()
        .enumerate()
        .map(|(position, id)| (*id, position))
        .collect();
    let mut out: Vec<SubjectSummary> = rows
        .into_iter()
        .filter(|row| order.contains_key(&row.id))
        .map(SubjectSummary::from)
        .collect();
    out.sort_by_key(|subject| order[&subject.id]);
    out.truncate(max_results as usize);
    Ok(Json(out))
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
    let language = input.language.unwrap_or_else(|| "en".to_string());
    let language = language.trim().to_string();
    if language.is_empty() || language.len() > 8 {
        return Err(AppError::BadRequest(
            "language must be 1-8 characters".to_string(),
        ));
    }
    let lesson_count = input.lesson_count.unwrap_or(20).clamp(1, 50);
    let description = input.description.unwrap_or_default();
    let tags = normalize_tags(input.tags.unwrap_or_default())?;
    let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
    // Grades 1-4, or 5 for high school and above ("5+" in the app).
    let grade = match input.grade {
        Some(g) if (1..=5).contains(&g) => Some(g),
        Some(_) => {
            return Err(AppError::BadRequest(
                "grade must be between 1 and 5".to_string(),
            ));
        }
        None => None,
    };
    let modality = match input.modality.as_deref() {
        None | Some("auto") => "auto",
        Some("text_only") => "text_only",
        Some(_) => {
            return Err(AppError::BadRequest(
                "modality must be 'auto' or 'text_only'".to_string(),
            ));
        }
    };
    let prerequisite_ids = normalize_prerequisites(input.prerequisite_ids.unwrap_or_default())?;
    check_prerequisites(&state.db, auth.0.id, &prerequisite_ids).await?;

    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO subjects (owner_id, title, description, language, visibility, status, lesson_count, tags_json, grade, modality)
         VALUES (?, ?, ?, ?, ?, 'ready', ?, ?, ?, ?) RETURNING id",
    )
    .bind(auth.0.id)
    .bind(&title)
    .bind(&description)
    .bind(&language)
    .bind("private")
    .bind(lesson_count)
    .bind(&tags_json)
    .bind(grade)
    .bind(modality)
    .fetch_one(&state.db)
    .await?;

    for prerequisite_id in &prerequisite_ids {
        sqlx::query(
            "INSERT INTO subject_prerequisites (subject_id, prerequisite_id) VALUES (?, ?)",
        )
        .bind(id)
        .bind(prerequisite_id)
        .execute(&state.db)
        .await?;
    }

    let row = sqlx::query_as::<_, SubjectRow>(&format!("SELECT {SUMMARY_COLUMNS} WHERE c.id = ?2"))
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

    let prerequisites = sqlx::query_as::<_, PrerequisiteRef>(
        "SELECT s.id, s.title, u.name AS owner_name FROM subject_prerequisites p
         JOIN subjects s ON s.id = p.prerequisite_id
         JOIN users u ON u.id = s.owner_id
         WHERE p.subject_id = ? ORDER BY p.rowid",
    )
    .bind(subject_id)
    .fetch_all(pool)
    .await?;

    Ok(SubjectDetail {
        subject,
        lessons,
        prerequisites,
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

/// Dedupes and validates prerequisite links: at most 8.
/// Links are set at creation only, so cycles are impossible: a new subject
/// has no incoming edges yet.
fn normalize_prerequisites(ids: Vec<i64>) -> Result<Vec<i64>, AppError> {
    if ids.len() > 8 {
        return Err(AppError::BadRequest(
            "at most 8 prerequisites allowed".to_string(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for id in ids {
        if seen.insert(id) {
            out.push(id);
        }
    }
    Ok(out)
}

/// Every prerequisite must exist, be visible to the creator, and be
/// published: only public subjects are immutable, so only they are safe
/// to build on. Unknown and hidden ids fail alike, so private subjects
/// of others cannot be probed or leak into AI context.
async fn check_prerequisites(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    ids: &[i64],
) -> Result<(), AppError> {
    if ids.is_empty() {
        return Ok(());
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT visibility FROM subjects
         WHERE id IN ({placeholders}) AND (visibility = 'public' OR owner_id = ?)"
    );
    let mut select = sqlx::query_scalar::<_, String>(&sql);
    for id in ids {
        select = select.bind(id);
    }
    let found: Vec<String> = select.bind(user_id).fetch_all(pool).await?;
    if found.len() != ids.len() {
        return Err(AppError::BadRequest(
            "a prerequisite subject was not found".to_string(),
        ));
    }
    if found.iter().any(|visibility| visibility != "public") {
        return Err(AppError::BadRequest(
            "prerequisites must be published subjects".to_string(),
        ));
    }
    Ok(())
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

/// Publishes an own, ready subject. Irreversible: a published subject can
/// no longer be regenerated or deleted, so learners never lose progress.
async fn publish_subject(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<SubjectSummary>, AppError> {
    let summary = subject_summary(&state.db, auth.0.id, id).await?;
    if !summary.mine {
        return Err(AppError::NotFound("subject not found".to_string()));
    }
    if summary.visibility == "public" {
        return Ok(Json(summary));
    }
    if summary.status != "ready" {
        return Err(AppError::BadRequest(
            "only ready subjects can be published".to_string(),
        ));
    }
    sqlx::query("UPDATE subjects SET visibility = 'public' WHERE id = ?")
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
    let summary = subject_summary(&state.db, auth.0.id, id).await?;
    if !summary.mine {
        return Err(AppError::NotFound("subject not found".to_string()));
    }
    if summary.visibility == "public" {
        return Err(AppError::BadRequest(
            "cannot delete a published subject".to_string(),
        ));
    }
    sqlx::query("DELETE FROM subjects WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    // Explicit even with ON DELETE CASCADE: links both ways must go.
    sqlx::query("DELETE FROM subject_prerequisites WHERE subject_id = ? OR prerequisite_id = ?")
        .bind(id)
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
