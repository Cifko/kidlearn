pub mod auth;
pub mod categories;
pub mod config;
pub mod db;
pub mod error;
pub mod generate;
pub mod meta;
pub mod models;
pub mod progress;
pub mod prompts;

use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub config: config::Config,
    pub http: reqwest::Client,
}

impl AppState {
    pub fn new(db: sqlx::SqlitePool, config: config::Config) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("HTTP client builds");
        Self { db, config, http }
    }
}

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .merge(auth::router())
        .merge(categories::router())
        .merge(generate::router())
        .merge(progress::router())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
