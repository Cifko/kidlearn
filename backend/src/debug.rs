use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{auth::AuthUser, error::AppError, AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/debug/llm-calls", get(list_calls))
}

#[derive(Debug, Default, Deserialize)]
struct ListQuery {
    subject_id: Option<i64>,
    kind: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct LlmCall {
    id: i64,
    created_at: String,
    kind: String,
    subject_id: Option<i64>,
    lesson_id: Option<i64>,
    asset_id: Option<i64>,
    label: String,
    model: String,
    duration_ms: i64,
    error: Option<String>,
    prompt: String,
    response: Option<String>,
}

/// Lists the caller's own recorded model calls, newest first, so bad
/// outputs can be traced back to the exact prompt that produced them.
async fn list_calls(
    auth: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<LlmCall>>, AppError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let rows = sqlx::query_as::<_, LlmCall>(
        "SELECT id, created_at, kind, subject_id, lesson_id, asset_id, label,
           model, duration_ms, error, prompt, response
         FROM llm_calls
         WHERE user_id = ?1
           AND (?2 IS NULL OR subject_id = ?2)
           AND (?3 IS NULL OR kind = ?3)
         ORDER BY id DESC LIMIT ?4",
    )
    .bind(auth.0.id)
    .bind(query.subject_id)
    .bind(query.kind)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}
