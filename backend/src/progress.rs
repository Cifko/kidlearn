use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{auth::AuthUser, error::AppError, models::Question, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/subcategories/{id}/lesson", get(get_lesson))
        .route("/subcategories/{id}/answer", post(answer_question))
        .route("/categories/{id}/progress", get(get_category_progress))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct SubWithCategory {
    id: i64,
    category_id: i64,
    position: i64,
    title: String,
    summary: String,
    owner_id: i64,
    visibility: String,
}

#[derive(Debug, Serialize)]
struct LessonOption {
    asset_key: String,
    svg_url: String,
}

#[derive(Debug, Serialize)]
struct LessonQuestion {
    id: i64,
    position: i64,
    qid: String,
    text: String,
    qtype: String,
    options: Vec<LessonOption>,
}

#[derive(Debug, Serialize)]
struct LessonSub {
    id: i64,
    category_id: i64,
    position: i64,
    title: String,
    summary: String,
}

#[derive(Debug, Serialize)]
struct Lesson {
    subcategory: LessonSub,
    questions: Vec<LessonQuestion>,
}

#[derive(Debug, Deserialize)]
struct AnswerRequest {
    question_id: i64,
    asset_key: String,
}

#[derive(Debug, Serialize)]
struct AnswerResponse {
    correct: bool,
    correct_asset_key: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct SubProgress {
    subcategory_id: i64,
    position: i64,
    total: i64,
    answered: i64,
    correct: i64,
}

#[derive(Debug, Serialize)]
struct SubProgressWithLock {
    #[serde(flatten)]
    progress: SubProgress,
    unlocked: bool,
}

#[derive(Debug, Serialize)]
struct CategoryProgress {
    category_id: i64,
    subcategories: Vec<SubProgressWithLock>,
}

async fn visible_sub(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    sub_id: i64,
) -> Result<SubWithCategory, AppError> {
    sqlx::query_as::<_, SubWithCategory>(
        "SELECT s.id, s.category_id, s.position, s.title, s.summary, c.owner_id, c.visibility
         FROM subcategories s JOIN categories c ON c.id = s.category_id WHERE s.id = ?",
    )
    .bind(sub_id)
    .fetch_optional(pool)
    .await?
    .filter(|sub| sub.visibility == "public" || sub.owner_id == user_id)
    .ok_or_else(|| AppError::NotFound("lesson not found".to_string()))
}

/// A lesson is unlocked when it is the first one or the previous lesson's
/// questions were all attempted.
async fn is_unlocked(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    category_id: i64,
    position: i64,
) -> Result<bool, AppError> {
    if position <= 1 {
        return Ok(true);
    }
    let prev: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM subcategories WHERE category_id = ? AND position < ?
         ORDER BY position DESC LIMIT 1",
    )
    .bind(category_id)
    .bind(position)
    .fetch_optional(pool)
    .await?;
    let Some((prev_id,)) = prev else {
        return Ok(true);
    };
    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM questions WHERE subcategory_id = ?")
        .bind(prev_id)
        .fetch_one(pool)
        .await?;
    let (answered,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM progress p JOIN questions q ON q.id = p.question_id
         WHERE q.subcategory_id = ? AND p.user_id = ?",
    )
    .bind(prev_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(answered >= total)
}

fn locked() -> AppError {
    AppError::Forbidden("complete the previous lesson first".to_string())
}

/// Returns a lesson's questions in order. Correct answers are never included;
/// the client must submit each answer for checking.
async fn get_lesson(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Lesson>, AppError> {
    let sub = visible_sub(&state.db, auth.0.id, id).await?;
    if !is_unlocked(&state.db, auth.0.id, sub.category_id, sub.position).await? {
        return Err(locked());
    }

    let assets: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, asset_key FROM assets WHERE subcategory_id = ?")
            .bind(sub.id)
            .fetch_all(&state.db)
            .await?;
    let asset_ids: HashMap<&str, i64> =
        assets.iter().map(|(id, key)| (key.as_str(), *id)).collect();

    let questions = sqlx::query_as::<_, Question>(
        "SELECT id, subcategory_id, position, qid, text, qtype, options_json, correct_asset
         FROM questions WHERE subcategory_id = ? ORDER BY position",
    )
    .bind(sub.id)
    .fetch_all(&state.db)
    .await?;

    let mut lesson_questions = Vec::new();
    for question in questions {
        let keys: Vec<String> = serde_json::from_str(&question.options_json).unwrap_or_default();
        let options: Vec<LessonOption> = keys
            .into_iter()
            .filter_map(|key| {
                asset_ids
                    .get(key.as_str())
                    .map(|asset_id| LessonOption {
                        asset_key: key,
                        svg_url: format!("/assets/{asset_id}/svg"),
                    })
            })
            .collect();
        if options.is_empty() {
            continue;
        }
        lesson_questions.push(LessonQuestion {
            id: question.id,
            position: question.position,
            qid: question.qid,
            text: question.text,
            qtype: question.qtype,
            options,
        });
    }

    Ok(Json(Lesson {
        subcategory: LessonSub {
            id: sub.id,
            category_id: sub.category_id,
            position: sub.position,
            title: sub.title,
            summary: sub.summary,
        },
        questions: lesson_questions,
    }))
}

/// Checks one answer and records it. Re-answering overwrites the attempt.
async fn answer_question(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<AnswerRequest>,
) -> Result<Json<AnswerResponse>, AppError> {
    let sub = visible_sub(&state.db, auth.0.id, id).await?;
    if !is_unlocked(&state.db, auth.0.id, sub.category_id, sub.position).await? {
        return Err(locked());
    }
    let question = sqlx::query_as::<_, Question>(
        "SELECT id, subcategory_id, position, qid, text, qtype, options_json, correct_asset
         FROM questions WHERE id = ? AND subcategory_id = ?",
    )
    .bind(input.question_id)
    .bind(sub.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("question not found".to_string()))?;

    let correct = question.correct_asset == input.asset_key;
    sqlx::query(
        "INSERT INTO progress (user_id, question_id, correct) VALUES (?, ?, ?)
         ON CONFLICT (user_id, question_id)
         DO UPDATE SET correct = excluded.correct, answered_at = datetime('now')",
    )
    .bind(auth.0.id)
    .bind(question.id)
    .bind(i64::from(correct))
    .execute(&state.db)
    .await?;

    Ok(Json(AnswerResponse {
        correct,
        correct_asset_key: question.correct_asset,
    }))
}

/// Per-subcategory progress for one category, with unlock flags.
async fn get_category_progress(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<CategoryProgress>, AppError> {
    let category: Option<(i64, String)> =
        sqlx::query_as("SELECT owner_id, visibility FROM categories WHERE id = ?")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    let Some((owner_id, visibility)) = category else {
        return Err(AppError::NotFound("category not found".to_string()));
    };
    if visibility != "public" && owner_id != auth.0.id {
        return Err(AppError::NotFound("category not found".to_string()));
    }

    let rows = sqlx::query_as::<_, SubProgress>(
        "SELECT s.id AS subcategory_id, s.position,
           COUNT(q.id) AS total,
           COUNT(p.question_id) AS answered,
           COALESCE(SUM(p.correct), 0) AS correct
         FROM subcategories s
         LEFT JOIN questions q ON q.subcategory_id = s.id
         LEFT JOIN progress p ON p.question_id = q.id AND p.user_id = ?
         WHERE s.category_id = ? GROUP BY s.id ORDER BY s.position",
    )
    .bind(auth.0.id)
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let mut subcategories = Vec::with_capacity(rows.len());
    let mut unlocked = true;
    for row in rows {
        let item_unlocked = unlocked;
        unlocked = row.answered >= row.total;
        subcategories.push(SubProgressWithLock {
            progress: row,
            unlocked: item_unlocked,
        });
    }

    Ok(Json(CategoryProgress {
        category_id: id,
        subcategories,
    }))
}
