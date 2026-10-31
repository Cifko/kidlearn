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
    let app = create_app(AppState::new(pool.clone(), config));
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

async fn create_subject_id(app: &axum::Router, token: &str) -> i64 {
    let response = app
        .clone()
        .oneshot(
            Request::post("/subjects")
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
    let id = create_subject_id(&app, &token).await;

    let response = app
        .oneshot(
            Request::post(format!("/subjects/{id}/generate"))
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

    // The subject is untouched: still ready, still empty.
    let (status,): (String,) = sqlx::query_as("SELECT status FROM subjects WHERE id = ?")
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
        "INSERT INTO subjects (owner_id, title, visibility)
         VALUES ((SELECT id FROM users WHERE email = 'other@example.com'), 'Theirs', 'public')
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let response = app
        .oneshot(
            Request::post(format!("/subjects/{other_id}/generate"))
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
    let subject_id = create_subject_id(&app, &token).await;

    let (sub_id,): (i64,) = sqlx::query_as(
        "INSERT INTO lessons (subject_id, position, title) VALUES (?, 1, 'Counting') RETURNING id",
    )
    .bind(subject_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let svg = r#"<svg viewBox="0 0 200 200"></svg>"#;
    let (asset_id,): (i64,) = sqlx::query_as(
        "INSERT INTO assets (lesson_id, asset_key, spec_json, svg_text) VALUES (?, 'red_ball', '{}', ?) RETURNING id",
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
    assert_eq!(response.headers()["content-type"], "image/svg+xml");
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
    let subject_id = create_subject_id(&app, &token).await;
    let (sub_id,): (i64,) = sqlx::query_as(
        "INSERT INTO lessons (subject_id, position, title) VALUES (?, 1, 'Counting') RETURNING id",
    )
    .bind(subject_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (asset_id,): (i64,) = sqlx::query_as(
        "INSERT INTO assets (lesson_id, asset_key, spec_json) VALUES (?, 'red_ball', '{}') RETURNING id",
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

#[tokio::test]
async fn generate_runs_in_background_and_reports_failure() {
    let mut config = Config::for_tests();
    config.meta_api_key = Some("test-key".to_string());
    config.meta_api_base = "http://127.0.0.1:1".to_string();
    let pool = db::init_pool(&config.database_url).await.unwrap();
    let app = create_app(AppState::new(pool.clone(), config));
    let token = login_token(&app).await;
    let id = create_subject_id(&app, &token).await;

    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/subjects/{id}/generate"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    // The background job fails fast (connection refused) and flips the status.
    let mut status = String::new();
    for _ in 0..100 {
        let response = app
            .clone()
            .oneshot(
                Request::get(format!("/subjects/{id}"))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), 65536)
            .await
            .unwrap();
        let detail: serde_json::Value = serde_json::from_slice(&body).unwrap();
        status = detail["status"].as_str().unwrap().to_string();
        if status != "generating" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(status, "failed");
}

async fn mock_lesson_backend(content: &str) -> (String, tokio::task::JoinHandle<()>) {
    let content = content.to_string();
    let app = axum::Router::new().route(
        "/chat/completions",
        axum::routing::post(move || {
            let content = content.clone();
            async move {
                axum::Json(json!({
                    "choices": [{ "message": { "content": content } }]
                }))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (url, handle)
}

fn init_logging() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("info")
            .with_test_writer()
            .try_init();
    });
}

#[tokio::test]
async fn generate_fans_out_and_tracks_progress() {
    init_logging();
    // One document serves both the plan and the lesson calls; each parser
    // reads its own keys and ignores the rest.
    let content = r##"{
        "lessons": [
            {"position": 1, "title": "L1", "summary": "first"},
            {"position": 2, "title": "L2", "summary": "second"},
            {"position": 3, "title": "L3", "summary": "third"}
        ],
        "questions": [
            {"id": "q1", "text": "Pick a", "type": "multiple_choice_image",
             "options_asset_ids": ["a", "b"], "correct_asset_id": "a"},
            {"id": "q2", "text": "Pick b", "type": "multiple_choice_image",
             "options_asset_ids": ["a", "b"], "correct_asset_id": "b"}
        ],
        "assets": [
            {"id": "a", "type": "real_object", "shape": null,
             "description": "A red ball", "main_color": "#ff0000", "secondary_color": "#ff8888"},
            {"id": "b", "type": "real_object", "shape": null,
             "description": "A blue cube", "main_color": "#0000ff", "secondary_color": "#8888ff"}
        ]
    }"##;
    let (url, _server) = mock_lesson_backend(content).await;
    let mut config = Config::for_tests();
    config.meta_api_key = Some("test-key".to_string());
    config.meta_api_base = url;
    let pool = db::init_pool(&config.database_url).await.unwrap();
    let app = create_app(AppState::new(pool, config));
    let token = login_token(&app).await;
    let id = create_subject_id(&app, &token).await;

    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/subjects/{id}/generate"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "lesson_count": 3, "min_questions": 1, "max_questions": 2 })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let mut detail = serde_json::Value::Null;
    for _ in 0..100 {
        let response = app
            .clone()
            .oneshot(
                Request::get(format!("/subjects/{id}"))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), 65536)
            .await
            .unwrap();
        detail = serde_json::from_slice(&body).unwrap();
        if detail["status"] != "generating" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(detail["status"], "ready");
    assert_eq!(detail["generated_lessons"], 3);
    let lessons = detail["lessons"].as_array().unwrap();
    assert_eq!(lessons.len(), 3);
    for (index, lesson) in lessons.iter().enumerate() {
        assert_eq!(lesson["position"], (index + 1) as i64);
        assert_eq!(lesson["question_count"], 2);
    }
}
