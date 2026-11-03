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
async fn image_serves_cached_bytes() {
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
    let png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00];
    let (asset_id,): (i64,) = sqlx::query_as(
        "INSERT INTO assets (lesson_id, asset_key, spec_json, image_blob, image_mime)
         VALUES (?, 'red_ball', '{}', ?, 'image/png') RETURNING id",
    )
    .bind(sub_id)
    .bind(&png)
    .fetch_one(&pool)
    .await
    .unwrap();

    let response = app
        .oneshot(
            Request::get(format!("/assets/{asset_id}/image"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/png");
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    assert_eq!(body.as_ref(), png.as_slice());
}

#[tokio::test]
async fn background_serves_cached_bytes() {
    let (pool, app) = test_state().await;
    let token = login_token(&app).await;
    let subject_id = create_subject_id(&app, &token).await;
    let png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00];
    sqlx::query(
        "UPDATE subjects SET background_blob = ?, background_mime = 'image/png' WHERE id = ?",
    )
    .bind(&png)
    .bind(subject_id)
    .execute(&pool)
    .await
    .unwrap();

    let response = app
        .oneshot(
            Request::get(format!("/subjects/{subject_id}/background"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/png");
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    assert_eq!(body.as_ref(), png.as_slice());
}

#[tokio::test]
async fn background_guards_others_private_subject() {
    let (_pool, app) = test_state().await;
    let token = login_token(&app).await;
    // Second dev user owns a private subject.
    let response = app
        .clone()
        .oneshot(
            Request::post("/auth/dev-login")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"who":"dev2"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let login: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let other = login["token"].as_str().unwrap().to_string();
    let hidden_id = create_subject_id(&app, &other).await;

    for id in [hidden_id, 999999] {
        let response = app
            .clone()
            .oneshot(
                Request::get(format!("/subjects/{id}/background"))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

/// Full generation prefills the background without any view request:
/// the recorded call appears on its own, then the endpoint serves it.
#[tokio::test]
async fn generate_prefills_subject_background() {
    let (url, _server) = mock_lesson_backend(LESSON_FIXTURE).await;
    let mut config = Config::for_tests();
    config.meta_api_key = Some("test-key".to_string());
    config.meta_api_base = url;
    let pool = db::init_pool(&config.database_url).await.unwrap();
    let app = create_app(AppState::new(pool, config));
    let token = login_token(&app).await;
    let id = create_subject_id(&app, &token).await;

    start_mock_generation(&app, &token, id).await;
    let detail = wait_until_settled(&app, &token, id).await;
    assert_eq!(detail["status"], "ready");

    let mut prompt = String::new();
    for _ in 0..100 {
        let calls = list_calls(&app, &token, "?kind=background").await;
        if !calls.as_array().unwrap().is_empty() {
            prompt = calls[0]["prompt"].as_str().unwrap().to_string();
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(prompt.contains("Math"), "prompt: {prompt}");
    assert!(prompt.contains("No text"), "prompt: {prompt}");

    let response = app
        .clone()
        .oneshot(
            Request::get(format!("/subjects/{id}/background"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/png");

    let lesson_id = detail["lessons"][0]["id"].as_i64().unwrap();
    let response = app
        .oneshot(
            Request::get(format!("/lessons/{lesson_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    let lesson: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        lesson["background_url"],
        format!("/subjects/{id}/background")
    );
}

/// Subjects created before backgrounds existed render on first request.
#[tokio::test]
async fn background_generates_lazily_on_first_request() {
    let (url, _server) = mock_lesson_backend(LESSON_FIXTURE).await;
    let mut config = Config::for_tests();
    config.meta_api_key = Some("test-key".to_string());
    config.meta_api_base = url;
    let pool = db::init_pool(&config.database_url).await.unwrap();
    let app = create_app(AppState::new(pool, config));
    let token = login_token(&app).await;
    let id = create_subject_id(&app, &token).await;

    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(
                Request::get(format!("/subjects/{id}/background"))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "image/png");
    }
    // Generated once, served from cache the second time.
    let calls = list_calls(&app, &token, "?kind=background").await;
    assert_eq!(calls.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn image_missing_asset_returns_404() {
    let (_pool, app) = test_state().await;
    let token = login_token(&app).await;
    let response = app
        .oneshot(
            Request::get("/assets/999/image")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn image_uncached_without_key_reports_disabled() {
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
            Request::get(format!("/assets/{asset_id}/image"))
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

/// A 1x1 transparent PNG, base64-encoded.
const PIXEL_PNG_B64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

async fn mock_lesson_backend(content: &str) -> (String, tokio::task::JoinHandle<()>) {
    let content = content.to_string();
    let app = axum::Router::new()
        .route(
            "/chat/completions",
            axum::routing::post(move || {
                let content = content.clone();
                async move {
                    axum::Json(json!({
                        "choices": [{ "message": { "content": content } }]
                    }))
                }
            }),
        )
        .route(
            "/images/generations",
            axum::routing::post(|| async {
                axum::Json(json!({ "data": [{ "b64_json": PIXEL_PNG_B64 }] }))
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

// One document serves both the plan and the lesson calls; each parser
// reads its own keys and ignores the rest.
const LESSON_FIXTURE: &str = r##"{
    "lessons": [
        {"position": 1, "title": "L1", "summary": "first"},
        {"position": 2, "title": "L2", "summary": "second"},
        {"position": 3, "title": "L3", "summary": "third"}
    ],
    "questions": [
        {"id": "q1", "text": "Pick a in the picture", "type": "multiple_choice_image", "focus": "name object a",
         "figure_asset_id": "a", "options_asset_ids": ["a", "b"], "correct_asset_id": "a"},
        {"id": "q2", "text": "Pick two", "type": "multiple_choice_text", "focus": "read number 2",
         "plot": {"functions": [{"expr": "x", "label": ""}], "x_min": 0, "x_max": 3, "y_min": 0, "y_max": 3, "points": []},
         "options_text": ["1", "2", "3"], "correct_text": "2"}
    ],
    "assets": [
        {"id": "a", "type": "real_object", "shape": null,
         "description": "A red ball", "main_color": "#ff0000", "secondary_color": "#ff8888"},
        {"id": "b", "type": "real_object", "shape": null,
         "description": "A blue cube", "main_color": "#0000ff", "secondary_color": "#8888ff"}
    ]
}"##;

#[tokio::test]
async fn generate_fans_out_and_tracks_progress() {
    init_logging();
    let (url, _server) = mock_lesson_backend(LESSON_FIXTURE).await;
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

async fn wait_until_settled(app: &axum::Router, token: &str, id: i64) -> serde_json::Value {
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
        if detail["status"] != "generating" {
            return detail;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("generation did not settle in time");
}

async fn list_calls(app: &axum::Router, token: &str, query: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::get(format!("/debug/llm-calls{query}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 262144)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn start_mock_generation(app: &axum::Router, token: &str, id: i64) {
    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/subjects/{id}/generate"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "lesson_count": 2, "min_questions": 1, "max_questions": 2 })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn generate_records_prompts_and_answers() {
    let (url, _server) = mock_lesson_backend(LESSON_FIXTURE).await;
    let mut config = Config::for_tests();
    config.meta_api_key = Some("test-key".to_string());
    config.meta_api_base = url;
    let pool = db::init_pool(&config.database_url).await.unwrap();
    let app = create_app(AppState::new(pool, config));
    let token = login_token(&app).await;
    let id = create_subject_id(&app, &token).await;
    start_mock_generation(&app, &token, id).await;
    let detail = wait_until_settled(&app, &token, id).await;
    assert_eq!(detail["status"], "ready");

    let plans = list_calls(&app, &token, "?kind=plan").await;
    let plans = plans.as_array().unwrap();
    assert_eq!(plans.len(), 1);
    assert!(plans[0]["prompt"].as_str().unwrap().contains("Math"));
    assert!(plans[0]["response"].as_str().unwrap().contains("L1"));
    assert_eq!(plans[0]["subject_id"], id);
    assert_eq!(plans[0]["model"], "test-model");

    let questions = list_calls(&app, &token, "?kind=questions").await;
    let questions = questions.as_array().unwrap();
    assert_eq!(questions.len(), 2);
    let mut labels: Vec<&str> = questions
        .iter()
        .map(|call| call["label"].as_str().unwrap())
        .collect();
    labels.sort_unstable();
    assert_eq!(labels, vec!["L1", "L2"]);
    assert!(questions[0]["response"]
        .as_str()
        .unwrap()
        .contains("Pick a"));

    // Image prefill runs after the subject flips to ready.
    for _ in 0..100 {
        let images = list_calls(&app, &token, "?kind=image").await;
        if images.as_array().unwrap().len() == 4 {
            assert!(images[0]["prompt"]
                .as_str()
                .unwrap()
                .contains("children's school books"));
            assert!(images[0]["asset_id"].as_i64().unwrap() > 0);
            // Pixels stay out of the log; only a summary is recorded.
            let summary = images[0]["response"].as_str().unwrap();
            assert!(summary.contains("image/png"), "summary: {summary}");
            assert!(summary.contains("bytes"), "summary: {summary}");
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("image prefill calls were not recorded in time");
}

#[tokio::test]
async fn text_only_generation_keeps_text_questions_only() {
    let (url, _server) = mock_lesson_backend(LESSON_FIXTURE).await;
    let mut config = Config::for_tests();
    config.meta_api_key = Some("test-key".to_string());
    config.meta_api_base = url;
    let pool = db::init_pool(&config.database_url).await.unwrap();
    let app = create_app(AppState::new(pool.clone(), config));
    let token = login_token(&app).await;

    let response = app
        .clone()
        .oneshot(
            Request::post("/subjects")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "title": "Math", "modality": "text_only" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = created["id"].as_i64().unwrap();

    start_mock_generation(&app, &token, id).await;
    let detail = wait_until_settled(&app, &token, id).await;
    assert_eq!(detail["status"], "ready");
    // The fixture's image question is dropped; only the text one survives.
    for lesson in detail["lessons"].as_array().unwrap() {
        assert_eq!(lesson["question_count"], 1);
    }
    let (assets,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM assets")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(assets, 0);
}

#[tokio::test]
async fn generate_records_errors() {
    let mut config = Config::for_tests();
    config.meta_api_key = Some("test-key".to_string());
    config.meta_api_base = "http://127.0.0.1:1".to_string();
    let pool = db::init_pool(&config.database_url).await.unwrap();
    let app = create_app(AppState::new(pool, config));
    let token = login_token(&app).await;
    let id = create_subject_id(&app, &token).await;
    start_mock_generation(&app, &token, id).await;
    let detail = wait_until_settled(&app, &token, id).await;
    assert_eq!(detail["status"], "failed");

    let calls = list_calls(&app, &token, "").await;
    let calls = calls.as_array().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["kind"], "plan");
    assert!(calls[0]["error"].as_str().unwrap().contains("unreachable"));
    assert!(calls[0]["response"].is_null());
}

async fn create_subject_body(app: &axum::Router, token: &str, body: serde_json::Value) -> i64 {
    let response = app
        .clone()
        .oneshot(
            Request::post("/subjects")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
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

async fn publish_subject(app: &axum::Router, token: &str, id: i64) {
    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/subjects/{id}/publish"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// Chain A -> B -> C: generating C feeds both ancestors' lesson plans
/// (direct and transitive) into the plan and questions prompts.
#[tokio::test]
async fn generate_includes_transitive_prerequisite_context() {
    let (url, _server) = mock_lesson_backend(LESSON_FIXTURE).await;
    let mut config = Config::for_tests();
    config.meta_api_key = Some("test-key".to_string());
    config.meta_api_base = url;
    let pool = db::init_pool(&config.database_url).await.unwrap();
    let app = create_app(AppState::new(pool.clone(), config));
    let token = login_token(&app).await;

    let a = create_subject_body(&app, &token, json!({ "title": "WW2" })).await;
    sqlx::query("INSERT INTO lessons (subject_id, position, title, summary) VALUES (?, 1, 'Causes', 'names the main causes')")
        .bind(a)
        .execute(&pool)
        .await
        .unwrap();
    publish_subject(&app, &token, a).await;
    let b = create_subject_body(
        &app,
        &token,
        json!({ "title": "WW2 battles", "prerequisite_ids": [a] }),
    )
    .await;
    sqlx::query("INSERT INTO lessons (subject_id, position, title, summary) VALUES (?, 1, 'Stalingrad', 'describes the turning point')")
        .bind(b)
        .execute(&pool)
        .await
        .unwrap();
    publish_subject(&app, &token, b).await;
    let c = create_subject_body(
        &app,
        &token,
        json!({ "title": "WW2 advanced", "prerequisite_ids": [b] }),
    )
    .await;

    start_mock_generation(&app, &token, c).await;
    let detail = wait_until_settled(&app, &token, c).await;
    assert_eq!(detail["status"], "ready");

    let plans = list_calls(&app, &token, "?kind=plan").await;
    let prompt = plans[0]["prompt"].as_str().unwrap();
    // Direct prerequisite first, then the transitive one.
    let battles = prompt.find("WW2 battles").unwrap();
    let ww2 = prompt.find("Prior subject \"WW2\"").unwrap();
    assert!(battles < ww2);
    assert!(prompt.contains("Stalingrad"));
    assert!(prompt.contains("Causes"));

    let questions = list_calls(&app, &token, "?kind=questions").await;
    let prompt = questions[0]["prompt"].as_str().unwrap();
    assert!(prompt.contains("Stalingrad"));
    assert!(prompt.contains("Causes"));
}
