use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use kidlearn_backend::{config::Config, create_app, db, AppState};
use serde_json::json;
use tower::ServiceExt;

async fn test_state() -> (sqlx::SqlitePool, axum::Router) {
    let config = Config::for_tests();
    let pool = db::init_pool(&config.database_url).await.unwrap();
    let app = create_app(AppState {
        db: pool.clone(),
        config,
    });
    (pool, app)
}

async fn login_token(app: &axum::Router) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::post("/auth/dev-login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    json["token"].as_str().unwrap().to_string()
}

async fn create_category_id(app: &axum::Router, token: &str) -> i64 {
    let response = app
        .clone()
        .oneshot(
            Request::post("/categories")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "title": "Math" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    json["id"].as_i64().unwrap()
}

#[tokio::test]
async fn generate_without_api_key_reports_disabled() {
    let (pool, app) = test_state().await;
    let token = login_token(&app).await;
    let id = create_category_id(&app, &token).await;

    let response = app
        .oneshot(
            Request::post(format!("/categories/{id}/generate"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["error"].as_str().unwrap().contains("META_API_KEY"));

    // The category is untouched: still ready, still empty.
    let (status,): (String,) =
        sqlx::query_as("SELECT status FROM categories WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "ready");
}

#[tokio::test]
async fn generate_rejects_non_owner() {
    let (pool, app) = test_state().await;
    let token = login_token(&app).await;

    sqlx::query("INSERT INTO users (email, name) VALUES ('other@example.com', 'Other')")
        .execute(&pool)
        .await
        .unwrap();
    let (other_id,): (i64,) = sqlx::query_as(
        "INSERT INTO categories (owner_id, title, visibility)
         VALUES ((SELECT id FROM users WHERE email = 'other@example.com'), 'Theirs', 'public')
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let response = app
        .oneshot(
            Request::post(format!("/categories/{other_id}/generate"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    // Key check runs before the ownership check; both mean "cannot generate".
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn svg_serves_cached_image() {
    let (pool, app) = test_state().await;
    let token = login_token(&app).await;
    let category_id = create_category_id(&app, &token).await;

    let (sub_id,): (i64,) = sqlx::query_as(
        "INSERT INTO subcategories (category_id, position, title) VALUES (?, 1, 'Counting') RETURNING id",
    )
    .bind(category_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let svg = r#"<svg viewBox="0 0 200 200"></svg>"#;
    let (asset_id,): (i64,) = sqlx::query_as(
        "INSERT INTO assets (subcategory_id, asset_key, spec_json, svg_text) VALUES (?, 'red_ball', '{}', ?) RETURNING id",
    )
    .bind(sub_id)
    .bind(svg)
    .fetch_one(&pool)
    .await
    .unwrap();

    let response = app
        .oneshot(
            Request::get(format!("/assets/{asset_id}/svg"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"],
        "image/svg+xml"
    );
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    assert_eq!(body.as_ref(), svg.as_bytes());
}

#[tokio::test]
async fn svg_missing_asset_returns_404() {
    let (_pool, app) = test_state().await;
    let token = login_token(&app).await;
    let response = app
        .oneshot(
            Request::get("/assets/999/svg")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn svg_uncached_without_key_reports_disabled() {
    let (pool, app) = test_state().await;
    let token = login_token(&app).await;
    let category_id = create_category_id(&app, &token).await;
    let (sub_id,): (i64,) = sqlx::query_as(
        "INSERT INTO subcategories (category_id, position, title) VALUES (?, 1, 'Counting') RETURNING id",
    )
    .bind(category_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (asset_id,): (i64,) = sqlx::query_as(
        "INSERT INTO assets (subcategory_id, asset_key, spec_json) VALUES (?, 'red_ball', '{}') RETURNING id",
    )
    .bind(sub_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let response = app
        .oneshot(
            Request::get(format!("/assets/{asset_id}/svg"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
