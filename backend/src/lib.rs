pub mod config;
pub mod db;

use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub config: config::Config,
}

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
