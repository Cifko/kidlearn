use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{auth::AuthUser, error::AppError, models::PlotSpec, models::Question, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/lessons/{id}", get(get_lesson))
        .route("/lessons/{id}/answer", post(answer_question))
        .route("/subjects/{id}/progress", get(get_subject_progress))
        .route("/stats", get(get_stats))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct LessonWithSubject {
    id: i64,
    subject_id: i64,
    position: i64,
    title: String,
    summary: String,
    owner_id: i64,
    visibility: String,
}

#[derive(Debug, Serialize)]
struct LessonOption {
    asset_key: Option<String>,
    image_url: Option<String>,
    text: Option<String>,
}

#[derive(Debug, Serialize)]
struct LessonQuestion {
    id: i64,
    position: i64,
    qid: String,
    text: String,
    qtype: String,
    image_url: Option<String>,
    plot: Option<PlotSpec>,
    options: Vec<LessonOption>,
}

#[derive(Debug, Serialize)]
struct Lesson {
    id: i64,
    subject_id: i64,
    position: i64,
    title: String,
    summary: String,
    questions: Vec<LessonQuestion>,
}

#[derive(Debug, Deserialize)]
struct AnswerRequest {
    question_id: i64,
    asset_key: Option<String>,
    text: Option<String>,
}

/// XP for the first correct attempt of a question. Re-answering earns nothing.
const XP_PER_CORRECT: i64 = 10;

#[derive(Debug, Serialize)]
struct AnswerResponse {
    correct: bool,
    correct_asset_key: String,
    correct_text: String,
    xp_earned: i64,
    total_xp: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct Stats {
    xp: i64,
    streak_days: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct LessonProgress {
    lesson_id: i64,
    position: i64,
    total: i64,
    answered: i64,
    correct: i64,
}

#[derive(Debug, Serialize)]
struct LessonProgressWithLock {
    #[serde(flatten)]
    progress: LessonProgress,
    unlocked: bool,
}

#[derive(Debug, Serialize)]
struct SubjectProgress {
    subject_id: i64,
    lessons: Vec<LessonProgressWithLock>,
}

async fn visible_sub(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    sub_id: i64,
) -> Result<LessonWithSubject, AppError> {
    sqlx::query_as::<_, LessonWithSubject>(
        "SELECT s.id, s.subject_id, s.position, s.title, s.summary, c.owner_id, c.visibility
         FROM lessons s JOIN subjects c ON c.id = s.subject_id WHERE s.id = ?",
    )
    .bind(sub_id)
    .fetch_optional(pool)
    .await?
    .filter(|sub| sub.visibility == "public" || sub.owner_id == user_id)
    .ok_or_else(|| AppError::NotFound("lesson not found".to_string()))
}

/// A lesson is unlocked when it is the first one or the previous lesson's
/// questions were all attempted. A lesson with no questions yet (still
/// generating) never unlocks the next one: 0/0 is not complete.
async fn is_unlocked(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    subject_id: i64,
    position: i64,
) -> Result<bool, AppError> {
    if position <= 1 {
        return Ok(true);
    }
    let prev: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM lessons WHERE subject_id = ? AND position < ?
         ORDER BY position DESC LIMIT 1",
    )
    .bind(subject_id)
    .bind(position)
    .fetch_optional(pool)
    .await?;
    let Some((prev_id,)) = prev else {
        return Ok(true);
    };
    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM questions WHERE lesson_id = ?")
        .bind(prev_id)
        .fetch_one(pool)
        .await?;
    let (answered,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM progress p JOIN questions q ON q.id = p.question_id
         WHERE q.lesson_id = ? AND p.user_id = ?",
    )
    .bind(prev_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(total > 0 && answered >= total)
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
    if !is_unlocked(&state.db, auth.0.id, sub.subject_id, sub.position).await? {
        return Err(locked());
    }

    let assets: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, asset_key FROM assets WHERE lesson_id = ?")
            .bind(sub.id)
            .fetch_all(&state.db)
            .await?;
    let asset_ids: HashMap<&str, i64> =
        assets.iter().map(|(id, key)| (key.as_str(), *id)).collect();

    let questions = sqlx::query_as::<_, Question>(
        "SELECT id, lesson_id, position, qid, text, qtype, options_json, correct_asset,
            options_text_json, correct_text, figure_asset, plot_json
         FROM questions WHERE lesson_id = ? ORDER BY position",
    )
    .bind(sub.id)
    .fetch_all(&state.db)
    .await?;

    let mut lesson_questions = Vec::new();
    for question in questions {
        let options: Vec<LessonOption> = if question.qtype == "multiple_choice_text" {
            let texts: Vec<String> =
                serde_json::from_str(&question.options_text_json).unwrap_or_default();
            texts
                .into_iter()
                .map(|text| LessonOption {
                    asset_key: None,
                    image_url: None,
                    text: Some(text),
                })
                .collect()
        } else {
            let keys: Vec<String> =
                serde_json::from_str(&question.options_json).unwrap_or_default();
            keys.into_iter()
                .filter_map(|key| {
                    asset_ids.get(key.as_str()).map(|asset_id| LessonOption {
                        asset_key: Some(key),
                        image_url: Some(format!("/assets/{asset_id}/image")),
                        text: None,
                    })
                })
                .collect()
        };
        if options.is_empty() {
            continue;
        }
        let image_url = if question.figure_asset.is_empty() {
            None
        } else {
            asset_ids
                .get(question.figure_asset.as_str())
                .map(|asset_id| format!("/assets/{asset_id}/image"))
        };
        let plot = if question.plot_json.is_empty() {
            None
        } else {
            serde_json::from_str(&question.plot_json).unwrap_or(None)
        };
        lesson_questions.push(LessonQuestion {
            id: question.id,
            position: question.position,
            qid: question.qid,
            text: question.text,
            qtype: question.qtype,
            image_url,
            plot,
            options,
        });
    }

    Ok(Json(Lesson {
        id: sub.id,
        subject_id: sub.subject_id,
        position: sub.position,
        title: sub.title,
        summary: sub.summary,
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
    if !is_unlocked(&state.db, auth.0.id, sub.subject_id, sub.position).await? {
        return Err(locked());
    }
    let question = sqlx::query_as::<_, Question>(
        "SELECT id, lesson_id, position, qid, text, qtype, options_json, correct_asset,
            options_text_json, correct_text, figure_asset, plot_json
         FROM questions WHERE id = ? AND lesson_id = ?",
    )
    .bind(input.question_id)
    .bind(sub.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("question not found".to_string()))?;

    let correct = match question.qtype.as_str() {
        "multiple_choice_text" => input.text.as_deref() == Some(question.correct_text.as_str()),
        "multiple_choice_image" => {
            input.asset_key.as_deref() == Some(question.correct_asset.as_str())
        }
        _ => false,
    };
    let previously_correct: Option<i64> =
        sqlx::query_scalar("SELECT correct FROM progress WHERE user_id = ? AND question_id = ?")
            .bind(auth.0.id)
            .bind(question.id)
            .fetch_optional(&state.db)
            .await?;
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

    let xp_earned = if correct && previously_correct != Some(1) {
        XP_PER_CORRECT
    } else {
        0
    };
    let stats = record_activity(&state.db, auth.0.id, xp_earned).await?;

    Ok(Json(AnswerResponse {
        correct,
        correct_asset_key: question.correct_asset,
        correct_text: question.correct_text,
        xp_earned,
        total_xp: stats.xp,
    }))
}

/// Adds XP and maintains the daily streak: same day keeps it, yesterday
/// extends it, any gap restarts it at one.
async fn record_activity(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    xp_earned: i64,
) -> Result<Stats, AppError> {
    sqlx::query(
        "INSERT INTO stats (user_id, xp, streak_days, last_active_date)
         VALUES (?, ?, 1, date('now'))
         ON CONFLICT (user_id) DO UPDATE SET
           xp = stats.xp + excluded.xp,
           streak_days = CASE
             WHEN stats.last_active_date = date('now') THEN stats.streak_days
             WHEN stats.last_active_date = date('now', '-1 day') THEN stats.streak_days + 1
             ELSE 1 END,
           last_active_date = date('now')",
    )
    .bind(user_id)
    .bind(xp_earned)
    .execute(pool)
    .await?;
    Ok(
        sqlx::query_as::<_, Stats>("SELECT xp, streak_days FROM stats WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(pool)
            .await?,
    )
}

async fn get_stats(auth: AuthUser, State(state): State<AppState>) -> Result<Json<Stats>, AppError> {
    let stats = sqlx::query_as::<_, Stats>("SELECT xp, streak_days FROM stats WHERE user_id = ?")
        .bind(auth.0.id)
        .fetch_optional(&state.db)
        .await?
        .unwrap_or(Stats {
            xp: 0,
            streak_days: 0,
        });
    Ok(Json(stats))
}

/// Per-lesson progress for one subject, with unlock flags.
async fn get_subject_progress(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<SubjectProgress>, AppError> {
    let subject: Option<(i64, String)> =
        sqlx::query_as("SELECT owner_id, visibility FROM subjects WHERE id = ?")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    let Some((owner_id, visibility)) = subject else {
        return Err(AppError::NotFound("subject not found".to_string()));
    };
    if visibility != "public" && owner_id != auth.0.id {
        return Err(AppError::NotFound("subject not found".to_string()));
    }

    let rows = sqlx::query_as::<_, LessonProgress>(
        "SELECT s.id AS lesson_id, s.position,
           COUNT(q.id) AS total,
           COUNT(p.question_id) AS answered,
           COALESCE(SUM(p.correct), 0) AS correct
         FROM lessons s
         LEFT JOIN questions q ON q.lesson_id = s.id
         LEFT JOIN progress p ON p.question_id = q.id AND p.user_id = ?
         WHERE s.subject_id = ? GROUP BY s.id ORDER BY s.position",
    )
    .bind(auth.0.id)
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let mut lessons = Vec::with_capacity(rows.len());
    let mut unlocked = true;
    for row in rows {
        let item_unlocked = unlocked;
        // Empty lessons (still generating) keep the rest of the chain locked.
        unlocked = row.total > 0 && row.answered >= row.total;
        lessons.push(LessonProgressWithLock {
            progress: row,
            unlocked: item_unlocked,
        });
    }

    Ok(Json(SubjectProgress {
        subject_id: id,
        lessons,
    }))
}
