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
        .route("/auth/google", post(google_login))
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

    Ok(issue_login(&state, &user))
}

fn issue_login(state: &AppState, user: &User) -> Json<Value> {
    let token = sign_token(user.id, &state.config.jwt_secret);
    Json(json!({ "token": token, "user": user }))
}

#[derive(Debug, Deserialize)]
struct GoogleLoginRequest {
    id_token: String,
}

/// Google's tokeninfo answers everything as strings.
#[derive(Debug, Deserialize)]
struct TokenInfo {
    aud: String,
    sub: String,
    email: String,
    #[serde(default)]
    email_verified: String,
    exp: String,
    #[serde(default)]
    name: Option<String>,
}

/// Exchanges a Google ID token for our own JWT. The token is verified with
/// Google on every login; we never trust client-provided emails.
async fn google_login(
    State(state): State<AppState>,
    Json(input): Json<GoogleLoginRequest>,
) -> Result<Json<Value>, AppError> {
    let client_id =
        state.config.google_client_id.clone().ok_or_else(|| {
            AppError::ServiceUnavailable("Google login is not configured".to_string())
        })?;
    if input.id_token.trim().is_empty() {
        return Err(AppError::BadRequest("id_token is required".to_string()));
    }
    let url = format!(
        "{}?id_token={}",
        state.config.google_tokeninfo_url,
        input.id_token.trim()
    );
    let response = state.http.get(&url).send().await.map_err(|err| {
        AppError::ServiceUnavailable(format!("Google verification unreachable: {err}"))
    })?;
    if !response.status().is_success() {
        return Err(AppError::Unauthorized);
    }
    let info: TokenInfo = response.json().await.map_err(|_| AppError::Unauthorized)?;
    if info.aud != client_id || info.email_verified != "true" {
        return Err(AppError::Unauthorized);
    }
    let exp: u64 = info.exp.parse().map_err(|_| AppError::Unauthorized)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if exp <= now {
        return Err(AppError::Unauthorized);
    }

    let name = info
        .name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| info.email.clone());
    sqlx::query(
        "INSERT INTO users (email, name, google_sub) VALUES (?, ?, ?)
         ON CONFLICT (email) DO UPDATE SET name = excluded.name, google_sub = excluded.google_sub",
    )
    .bind(&info.email)
    .bind(&name)
    .bind(&info.sub)
    .execute(&state.db)
    .await?;
    let user = sqlx::query_as::<_, User>(
        "SELECT id, email, name, created_at FROM users WHERE email = ?",
    )
    .bind(&info.email)
    .fetch_one(&state.db)
    .await?;
    Ok(issue_login(&state, &user))
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
