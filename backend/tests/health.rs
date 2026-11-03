use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use kidlearn_backend::{config::Config, create_app, db, AppState};
use tower::ServiceExt;

async fn test_app() -> axum::Router {
    let config = Config::for_tests();
    let pool = db::init_pool(&config.database_url).await.unwrap();
    create_app(AppState::new(pool, config))
}

#[tokio::test]
async fn health_returns_ok() {
    let app = test_app().await;
    let response = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json, serde_json::json!({ "status": "ok" }));
}

#[tokio::test]
async fn database_schema_creates_all_tables() {
    let config = Config::for_tests();
    let pool = db::init_pool(&config.database_url).await.unwrap();
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        tables,
        vec![
            "assets",
            "lessons",
            "likes",
            "llm_calls",
            "progress",
            "questions",
            "stats",
            "subject_prerequisites",
            "subjects",
            "users"
        ]
    );
}
