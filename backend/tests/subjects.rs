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
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    json["token"].as_str().unwrap().to_string()
}

async fn create_subject(
    app: &axum::Router,
    token: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
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
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    (status, json)
}

#[tokio::test]
async fn create_and_list_own_subject() {
    let (_pool, app) = test_state().await;
    let token = login_token(&app).await;
    let (status, created) = create_subject(
        &app,
        &token,
        json!({ "title": "Prvouka", "description": "ucivo pre skolu", "language": "sk" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["title"], "Prvouka");
    assert_eq!(created["language"], "sk");
    assert_eq!(created["mine"], true);

    let response = app
        .oneshot(
            Request::get("/subjects")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["title"], "Prvouka");
}

#[tokio::test]
async fn list_filters_by_language() {
    let (_pool, app) = test_state().await;
    let token = login_token(&app).await;
    create_subject(&app, &token, json!({ "title": "Math", "language": "en" })).await;
    create_subject(
        &app,
        &token,
        json!({ "title": "Prvouka", "language": "sk" }),
    )
    .await;

    for (query, expected) in [
        ("/subjects?lang=sk", "Prvouka"),
        ("/subjects?lang=en", "Math"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::get(query)
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 8192)
            .await
            .unwrap();
        let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let titles: Vec<&str> = list
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["title"].as_str().unwrap())
            .collect();
        assert_eq!(titles, vec![expected]);
    }

    // Search and language combine.
    let response = app
        .oneshot(
            Request::get("/subjects?q=prvouka&lang=en")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(list.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn create_validates_input() {
    let (_pool, app) = test_state().await;
    let token = login_token(&app).await;

    let (status, _) = create_subject(&app, &token, json!({ "title": "  " })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_stores_and_validates_grade() {
    let (_pool, app) = test_state().await;
    let token = login_token(&app).await;

    let (status, created) = create_subject(&app, &token, json!({ "title": "No grade" })).await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(created["grade"].is_null());

    let (status, created) =
        create_subject(&app, &token, json!({ "title": "Graded", "grade": 2 })).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["grade"], 2);

    let (status, created) =
        create_subject(&app, &token, json!({ "title": "Older", "grade": 5 })).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["grade"], 5);

    for bad in [0, 6, -1] {
        let (status, _) =
            create_subject(&app, &token, json!({ "title": "Bad grade", "grade": bad })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn create_defaults_and_validates_modality() {
    let (_pool, app) = test_state().await;
    let token = login_token(&app).await;

    let (status, created) = create_subject(&app, &token, json!({ "title": "Auto" })).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["modality"], "auto");

    let (status, created) = create_subject(
        &app,
        &token,
        json!({ "title": "Text", "modality": "text_only" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["modality"], "text_only");

    let (status, _) = create_subject(
        &app,
        &token,
        json!({ "title": "Bad", "modality": "pictures_only" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_shows_public_but_hides_others_private() {
    let (pool, app) = test_state().await;
    let token = login_token(&app).await;

    sqlx::query("INSERT INTO users (email, name) VALUES ('other@example.com', 'Other')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO subjects (owner_id, title, visibility) VALUES
         ((SELECT id FROM users WHERE email = 'other@example.com'), 'Shared', 'public'),
         ((SELECT id FROM users WHERE email = 'other@example.com'), 'Secret', 'private')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let response = app
        .oneshot(
            Request::get("/subjects")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let titles: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Shared"));
    assert!(!titles.contains(&"Secret"));
}

#[tokio::test]
async fn detail_guards_others_private_subject() {
    let (pool, app) = test_state().await;
    let token = login_token(&app).await;

    sqlx::query("INSERT INTO users (email, name) VALUES ('other@example.com', 'Other')")
        .execute(&pool)
        .await
        .unwrap();
    let (private_id,): (i64,) = sqlx::query_as(
        "INSERT INTO subjects (owner_id, title, visibility)
         VALUES ((SELECT id FROM users WHERE email = 'other@example.com'), 'Secret', 'private')
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (public_id,): (i64,) = sqlx::query_as(
        "INSERT INTO subjects (owner_id, title, visibility)
         VALUES ((SELECT id FROM users WHERE email = 'other@example.com'), 'Shared', 'public')
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::get(format!("/subjects/{private_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = app
        .oneshot(
            Request::get(format!("/subjects/{public_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    let detail: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(detail["title"], "Shared");
    assert_eq!(detail["mine"], false);
    assert_eq!(detail["lessons"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn delete_removes_own_subject_only() {
    let (pool, app) = test_state().await;
    let token = login_token(&app).await;
    let (status, created) = create_subject(&app, &token, json!({ "title": "Temp" })).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_i64().unwrap();

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
        .clone()
        .oneshot(
            Request::delete(format!("/subjects/{other_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = app
        .clone()
        .oneshot(
            Request::delete(format!("/subjects/{id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = app
        .oneshot(
            Request::get(format!("/subjects/{id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn search_filters_visible_subjects() {
    let (pool, app) = test_state().await;
    let token = login_token(&app).await;
    create_subject(
        &app,
        &token,
        json!({ "title": "Prvouka", "description": "animals and plants" }),
    )
    .await;
    create_subject(
        &app,
        &token,
        json!({ "title": "Math", "description": "counting to ten" }),
    )
    .await;

    sqlx::query("INSERT INTO users (email, name) VALUES ('other@example.com', 'Other')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO subjects (owner_id, title, description, visibility) VALUES
         ((SELECT id FROM users WHERE email = 'other@example.com'), 'Shared animals', 'wildlife', 'public'),
         ((SELECT id FROM users WHERE email = 'other@example.com'), 'Secret animals', 'pets', 'private')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::get("/subjects?q=animals")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let mut titles: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap())
        .collect();
    titles.sort_unstable();
    assert_eq!(titles, vec!["Prvouka", "Shared animals"]);

    // LIKE wildcards in the query are literal, not patterns.
    let response = app
        .oneshot(
            Request::get("/subjects?q=%")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn subjects_require_auth() {
    let (_pool, app) = test_state().await;
    let response = app
        .oneshot(Request::get("/subjects").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

async fn like(app: &axum::Router, token: &str, id: i64) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/subjects/{id}/like"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

async fn unlike(app: &axum::Router, token: &str, id: i64) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::delete(format!("/subjects/{id}/like"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

async fn publish(app: &axum::Router, token: &str, id: i64) -> (StatusCode, serde_json::Value) {
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
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

#[tokio::test]
async fn like_unlike_flow_updates_counts() {
    let (pool, app) = test_state().await;
    let token = login_token(&app).await;

    sqlx::query("INSERT INTO users (email, name) VALUES ('other@example.com', 'Other')")
        .execute(&pool)
        .await
        .unwrap();
    let (shared_id,): (i64,) = sqlx::query_as(
        "INSERT INTO subjects (owner_id, title, visibility)
         VALUES ((SELECT id FROM users WHERE email = 'other@example.com'), 'Shared', 'public')
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let (status, liked) = like(&app, &token, shared_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(liked["like_count"], 1);
    assert_eq!(liked["liked_by_me"], true);

    // Liking twice is idempotent.
    let (status, liked) = like(&app, &token, shared_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(liked["like_count"], 1);

    let (status, unliked) = unlike(&app, &token, shared_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(unliked["like_count"], 0);
    assert_eq!(unliked["liked_by_me"], false);
}

#[tokio::test]
async fn like_rejects_own_and_hidden_subjects() {
    let (pool, app) = test_state().await;
    let token = login_token(&app).await;
    let (_, created) = create_subject(&app, &token, json!({ "title": "Mine" })).await;
    let mine_id = created["id"].as_i64().unwrap();

    let (status, _) = like(&app, &token, mine_id).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    sqlx::query("INSERT INTO users (email, name) VALUES ('other@example.com', 'Other')")
        .execute(&pool)
        .await
        .unwrap();
    let (secret_id,): (i64,) = sqlx::query_as(
        "INSERT INTO subjects (owner_id, title, visibility)
         VALUES ((SELECT id FROM users WHERE email = 'other@example.com'), 'Secret', 'private')
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (status, _) = like(&app, &token, secret_id).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_normalizes_and_returns_tags() {
    let (_pool, app) = test_state().await;
    let token = login_token(&app).await;
    let (status, created) = create_subject(
        &app,
        &token,
        json!({ "title": "Math", "tags": ["First Grade", " math ", "MATH"] }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["tags"], json!(["first grade", "math"]));
    let id = created["id"].as_i64().unwrap();

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
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    let detail: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(detail["tags"], json!(["first grade", "math"]));

    // Plain search matches tags too.
    let response = app
        .oneshot(
            Request::get("/subjects?q=grade")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["title"], "Math");
}

#[tokio::test]
async fn create_rejects_bad_tags() {
    let (_pool, app) = test_state().await;
    let token = login_token(&app).await;

    let (status, _) = create_subject(
        &app,
        &token,
        json!({ "title": "Bad", "tags": ["this tag is way too long for sure"] }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let many: Vec<String> = (0..9).map(|i| format!("tag{i}")).collect();
    let (status, _) = create_subject(&app, &token, json!({ "title": "Bad", "tags": many })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

async fn mock_meta(content: &str) -> (String, tokio::task::JoinHandle<()>) {
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

async fn ai_app(meta_base: &str) -> axum::Router {
    let mut config = Config::for_tests();
    config.meta_api_key = Some("test-key".to_string());
    config.meta_api_base = meta_base.to_string();
    let pool = db::init_pool(&config.database_url).await.unwrap();
    create_app(AppState::new(pool, config))
}

#[tokio::test]
async fn ai_search_returns_model_ranking() {
    // Model ranks third subject first, invents id 999 (dropped), then first.
    let (url, _server) = mock_meta(r#"{"ids":[3,999,1]}"#).await;
    let app = ai_app(&url).await;
    let token = login_token(&app).await;
    for title in ["Math", "Animals", "Prvouka"] {
        create_subject(&app, &token, json!({ "title": title })).await;
    }

    let response = app
        .oneshot(
            Request::get("/subjects/ai-search?q=school")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 16384)
        .await
        .unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let ids: Vec<i64> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids, vec![3, 1]);
}

#[tokio::test]
async fn ai_search_disabled_without_key() {
    let (_pool, app) = test_state().await;
    let token = login_token(&app).await;
    let response = app
        .oneshot(
            Request::get("/subjects/ai-search?q=school")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn ai_search_requires_query() {
    let (_pool, app) = test_state().await;
    let token = login_token(&app).await;
    let response = app
        .oneshot(
            Request::get("/subjects/ai-search")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn creation_is_always_private() {
    let (_pool, app) = test_state().await;
    let token = login_token(&app).await;
    // Even a client still sending the old visibility field gets a private subject.
    let (status, created) = create_subject(
        &app,
        &token,
        json!({ "title": "Mine", "visibility": "public" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["visibility"], "private");
}

#[tokio::test]
async fn publish_flow() {
    let (pool, app) = test_state().await;
    let token = login_token(&app).await;
    let (_, created) = create_subject(&app, &token, json!({ "title": "Mine" })).await;
    let id = created["id"].as_i64().unwrap();

    // A subject that is still generating cannot be published.
    sqlx::query("UPDATE subjects SET status = 'generating' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, _) = publish(&app, &token, id).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    sqlx::query("UPDATE subjects SET status = 'ready' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, published) = publish(&app, &token, id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(published["visibility"], "public");

    // Publishing twice is harmless.
    let (status, published) = publish(&app, &token, id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(published["visibility"], "public");
}

#[tokio::test]
async fn others_cannot_publish_your_subject() {
    let (_pool, app) = test_state().await;
    let mine = login_token(&app).await;
    let (_, created) = create_subject(&app, &mine, json!({ "title": "Mine" })).await;
    let id = created["id"].as_i64().unwrap();

    // Log in as the second dev user and try to publish someone else's subject.
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
    let other = login["token"].as_str().unwrap();

    let (status, _) = publish(&app, other, id).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn published_subjects_cannot_regenerate_or_delete() {
    // Point at nothing: regeneration must be rejected before any Meta call.
    let app = ai_app("http://127.0.0.1:1").await;
    let token = login_token(&app).await;
    let (_, created) = create_subject(&app, &token, json!({ "title": "Mine" })).await;
    let id = created["id"].as_i64().unwrap();
    let (status, _) = publish(&app, &token, id).await;
    assert_eq!(status, StatusCode::OK);

    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/subjects/{id}/generate"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = app
        .oneshot(
            Request::delete(format!("/subjects/{id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
