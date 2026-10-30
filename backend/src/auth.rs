use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{FromRequestParts, State},
    http::{header, request::Parts},
    routing::{get, post},
    Json, Router,
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{error::AppError, models::User, AppState};

/// Token lifetime: 30 days. Dev login only; Google OAuth comes later.
const TOKEN_TTL_SECONDS: u64 = 30 * 24 * 60 * 60;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: i64,
    exp: usize,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/dev-login", post(dev_login))
        .route("/me", get(me))
}

/// Development-only login: one shared dev user, no password, no OAuth.
async fn dev_login(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    sqlx::query(
        "INSERT INTO users (email, name) VALUES ('dev@localhost', 'Dev')
         ON CONFLICT (email) DO UPDATE SET name = excluded.name",
    )
    .execute(&state.db)
    .await?;

    let user = sqlx::query_as::<_, User>("SELECT id, email, name, created_at FROM users WHERE email = 'dev@localhost'")
        .fetch_one(&state.db)
        .await?;

    let token = sign_token(user.id, &state.config.jwt_secret);
    Ok(Json(json!({ "token": token, "user": user })))
}

async fn me(auth: AuthUser) -> Json<User> {
    Json(auth.0)
}

fn sign_token(user_id: i64, secret: &str) -> String {
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_secs()
        + TOKEN_TTL_SECONDS;
    let claims = Claims {
        sub: user_id,
        exp: exp as usize,
    };
    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .expect("JWT signing cannot fail with HMAC")
}

fn verify_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| AppError::Unauthorized)
}

/// Extractor for routes that require a logged-in user.
pub struct AuthUser(pub User);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(AppError::Unauthorized)?;
        let token = header.strip_prefix("Bearer ").ok_or(AppError::Unauthorized)?;
        let claims = verify_token(token, &state.config.jwt_secret)?;
        let user = sqlx::query_as::<_, User>(
            "SELECT id, email, name, created_at FROM users WHERE id = ?",
        )
        .bind(claims.sub)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;
        Ok(AuthUser(user))
    }
}
