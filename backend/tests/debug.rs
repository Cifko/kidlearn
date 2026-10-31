use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use kidlearn_backend::{config::Config, create_app, db, AppState};
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

async fn insert_call(
    pool: &sqlx::SqlitePool,
    email: &str,
    kind: &str,
    subject_id: Option<i64>,
    label: &str,
) {
    sqlx::query(
        "INSERT INTO llm_calls (user_id, kind, subject_id, label, model, prompt)
         VALUES ((SELECT id FROM users WHERE email = ?), ?, ?, ?, 'test-model', 'PROMPT')",
    )
    .bind(email)
    .bind(kind)
    .bind(subject_id)
    .bind(label)
    .execute(pool)
    .await
    .unwrap();
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
    let body = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn debug_lists_own_calls_newest_first() {
    let (pool, app) = test_state().await;
    let token = login_token(&app).await;
    sqlx::query("INSERT INTO users (email, name) VALUES ('other@example.com', 'Other')")
        .execute(&pool)
        .await
        .unwrap();

    insert_call(&pool, "dev@localhost", "plan", Some(1), "Math").await;
    insert_call(&pool, "dev@localhost", "questions", Some(1), "L1").await;
    insert_call(&pool, "other@example.com", "plan", Some(9), "Theirs").await;

    let calls = list_calls(&app, &token, "").await;
    let calls = calls.as_array().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls[0]["id"].as_i64().unwrap() > calls[1]["id"].as_i64().unwrap());
    assert_eq!(calls[0]["kind"], "questions");
    assert_eq!(calls[0]["label"], "L1");
    assert_eq!(calls[0]["prompt"], "PROMPT");
}

#[tokio::test]
async fn debug_filters_by_kind_subject_and_limit() {
    let (pool, app) = test_state().await;
    let token = login_token(&app).await;
    insert_call(&pool, "dev@localhost", "plan", Some(1), "Math").await;
    insert_call(&pool, "dev@localhost", "questions", Some(1), "L1").await;
    insert_call(&pool, "dev@localhost", "questions", Some(2), "L2").await;

    let calls = list_calls(&app, &token, "?kind=questions").await;
    assert_eq!(calls.as_array().unwrap().len(), 2);

    let calls = list_calls(&app, &token, "?subject_id=2").await;
    let calls = calls.as_array().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["label"], "L2");

    let calls = list_calls(&app, &token, "?limit=1").await;
    assert_eq!(calls.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn debug_requires_auth() {
    let (_pool, app) = test_state().await;
    let response = app
        .oneshot(
            Request::get("/debug/llm-calls")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
