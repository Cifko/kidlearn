use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{FromRequestParts, State},
    http::{header, request::Parts},
    routing::{get, post},
    Json, Router,
};
use hmac::{Hmac, Mac};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;

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
        .route("/auth/facebook", post(facebook_login))
        .route("/me", get(me))
}

#[derive(Debug, Default, Deserialize)]
struct DevLoginRequest {
    who: Option<String>,
}

/// Development-only login, no password, no OAuth. `{"who": "dev2"}` logs in
/// a second user so multi-user flows (likes, public subjects) are testable.
async fn dev_login(
    State(state): State<AppState>,
    body: Option<Json<DevLoginRequest>>,
) -> Result<Json<Value>, AppError> {
    let (email, name) = match body.and_then(|body| body.0.who) {
        Some(who) if who == "dev2" => ("dev2@localhost", "Dev 2"),
        _ => ("dev@localhost", "Dev"),
    };
    sqlx::query(
        "INSERT INTO users (email, name) VALUES (?, ?)
         ON CONFLICT (email) DO UPDATE SET name = excluded.name",
    )
    .bind(email)
    .bind(name)
    .execute(&state.db)
    .await?;

    let user =
        sqlx::query_as::<_, User>("SELECT id, email, name, created_at FROM users WHERE email = ?")
            .bind(email)
            .fetch_one(&state.db)
            .await?;

    tracing::info!(user_id = user.id, "dev login");
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
    let Some(client_id) = state.config.google_client_id.clone() else {
        tracing::warn!("google login attempted but GOOGLE_CLIENT_ID is not set");
        return Err(AppError::ServiceUnavailable(
            "Google login is not configured".to_string(),
        ));
    };
    if input.id_token.trim().is_empty() {
        return Err(AppError::BadRequest("id_token is required".to_string()));
    }
    let url = format!(
        "{}?id_token={}",
        state.config.google_tokeninfo_url,
        input.id_token.trim()
    );
    let response = state.http.get(&url).send().await.map_err(|err| {
        tracing::error!(%err, "google verification unreachable");
        AppError::ServiceUnavailable(format!("Google verification unreachable: {err}"))
    })?;
    if !response.status().is_success() {
        tracing::warn!("google rejected the id token");
        return Err(AppError::Unauthorized);
    }
    let info: TokenInfo = response.json().await.map_err(|_| AppError::Unauthorized)?;
    if info.aud != client_id {
        tracing::warn!("google token has wrong audience");
        return Err(AppError::Unauthorized);
    }
    if info.email_verified != "true" {
        tracing::warn!("google token email not verified");
        return Err(AppError::Unauthorized);
    }
    let exp: u64 = info.exp.parse().map_err(|_| AppError::Unauthorized)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if exp <= now {
        tracing::warn!("google token expired");
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
    let user =
        sqlx::query_as::<_, User>("SELECT id, email, name, created_at FROM users WHERE email = ?")
            .bind(&info.email)
            .fetch_one(&state.db)
            .await?;
    tracing::info!(user_id = user.id, "google login");
    Ok(issue_login(&state, &user))
}

#[derive(Debug, Deserialize)]
struct FacebookLoginRequest {
    code: String,
    redirect_uri: String,
}

#[derive(Debug, Deserialize)]
struct FacebookTokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct FacebookUserInfo {
    id: String,
    name: Option<String>,
    email: Option<String>,
}

/// Exchanges a Facebook Login code for a session JWT. The app secret
/// never leaves the server: the app only ever sees the short-lived code.
async fn facebook_login(
    State(state): State<AppState>,
    Json(input): Json<FacebookLoginRequest>,
) -> Result<Json<Value>, AppError> {
    let (app_id, app_secret) = match (
        state.config.facebook_app_id.clone(),
        state.config.facebook_app_secret.clone(),
    ) {
        (Some(app_id), Some(app_secret)) => (app_id, app_secret),
        _ => {
            tracing::warn!("facebook login attempted but app is not configured");
            return Err(AppError::ServiceUnavailable(
                "Facebook login is not configured".to_string(),
            ));
        }
    };
    if input.code.trim().is_empty() || input.redirect_uri.trim().is_empty() {
        return Err(AppError::BadRequest(
            "code and redirect_uri are required".to_string(),
        ));
    }
    let graph = state.config.facebook_graph_url.clone();
    let token: FacebookTokenResponse = state
        .http
        .get(format!("{graph}/oauth/access_token"))
        .query(&[
            ("client_id", app_id.as_str()),
            ("redirect_uri", input.redirect_uri.trim()),
            ("client_secret", app_secret.as_str()),
            ("code", input.code.trim()),
        ])
        .send()
        .await
        .map_err(|err| {
            tracing::error!(%err, "facebook exchange unreachable");
            AppError::ServiceUnavailable(format!("Facebook unreachable: {err}"))
        })?
        .error_for_status()
        .map_err(|_| AppError::Unauthorized)?
        .json()
        .await
        .map_err(|_| AppError::Unauthorized)?;

    // appsecret_proof binds the token to this app (Facebook's CSRF guard).
    let mut mac = Hmac::<Sha256>::new_from_slice(app_secret.as_bytes())
        .map_err(|_| AppError::Internal("invalid Facebook app secret".to_string()))?;
    mac.update(token.access_token.as_bytes());
    let proof = hex_encode(&mac.finalize().into_bytes());

    let info: FacebookUserInfo = state
        .http
        .get(format!("{graph}/me"))
        .query(&[
            ("fields", "id,name,email"),
            ("access_token", token.access_token.as_str()),
            ("appsecret_proof", proof.as_str()),
        ])
        .send()
        .await
        .map_err(|_| AppError::Unauthorized)?
        .error_for_status()
        .map_err(|_| AppError::Unauthorized)?
        .json()
        .await
        .map_err(|_| AppError::Unauthorized)?;
    let email = info
        .email
        .ok_or_else(|| AppError::BadRequest("Facebook account has no email address".to_string()))?;
    // Same verified-email linking as Google: a matching email adopts the
    // Facebook subject instead of creating a duplicate user.
    let name = info
        .name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| email.clone());
    sqlx::query(
        "INSERT INTO users (email, name, facebook_sub) VALUES (?, ?, ?)
         ON CONFLICT (email) DO UPDATE SET name = excluded.name, facebook_sub = excluded.facebook_sub",
    )
    .bind(&email)
    .bind(&name)
    .bind(&info.id)
    .execute(&state.db)
    .await?;
    let user =
        sqlx::query_as::<_, User>("SELECT id, email, name, created_at FROM users WHERE email = ?")
            .bind(&email)
            .fetch_one(&state.db)
            .await?;
    tracing::info!(user_id = user.id, "facebook login");
    Ok(issue_login(&state, &user))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
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
        let token = header
            .strip_prefix("Bearer ")
            .ok_or(AppError::Unauthorized)?;
        let claims = verify_token(token, &state.config.jwt_secret)?;
        let user =
            sqlx::query_as::<_, User>("SELECT id, email, name, created_at FROM users WHERE id = ?")
                .bind(claims.sub)
                .fetch_optional(&state.db)
                .await?
                .ok_or(AppError::Unauthorized)?;
        Ok(AuthUser(user))
    }
}
