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

async fn create_category(app: &axum::Router, token: &str, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/categories")
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
async fn create_and_list_own_category() {
    let (_pool, app) = test_state().await;
    let token = login_token(&app).await;
    let (status, created) = create_category(
        &app,
        &token,
        json!({ "title": "Prvouka", "description": "ucivo pre skolu", "language": "sk", "visibility": "private" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["title"], "Prvouka");
    assert_eq!(created["language"], "sk");
    assert_eq!(created["mine"], true);

    let response = app
        .oneshot(
            Request::get("/categories")
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
async fn create_validates_input() {
    let (_pool, app) = test_state().await;
    let token = login_token(&app).await;

    let (status, _) = create_category(&app, &token, json!({ "title": "  " })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = create_category(
        &app,
        &token,
        json!({ "title": "Ok", "visibility": "everyone" }),
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
        "INSERT INTO categories (owner_id, title, visibility) VALUES
         ((SELECT id FROM users WHERE email = 'other@example.com'), 'Shared', 'public'),
         ((SELECT id FROM users WHERE email = 'other@example.com'), 'Secret', 'private')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let response = app
        .oneshot(
            Request::get("/categories")
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
async fn detail_guards_others_private_category() {
    let (pool, app) = test_state().await;
    let token = login_token(&app).await;

    sqlx::query("INSERT INTO users (email, name) VALUES ('other@example.com', 'Other')")
        .execute(&pool)
        .await
        .unwrap();
    let (private_id,): (i64,) = sqlx::query_as(
        "INSERT INTO categories (owner_id, title, visibility)
         VALUES ((SELECT id FROM users WHERE email = 'other@example.com'), 'Secret', 'private')
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (public_id,): (i64,) = sqlx::query_as(
        "INSERT INTO categories (owner_id, title, visibility)
         VALUES ((SELECT id FROM users WHERE email = 'other@example.com'), 'Shared', 'public')
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::get(format!("/categories/{private_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = app
        .oneshot(
            Request::get(format!("/categories/{public_id}"))
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
    assert_eq!(detail["subcategories"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn delete_removes_own_category_only() {
    let (pool, app) = test_state().await;
    let token = login_token(&app).await;
    let (status, created) = create_category(&app, &token, json!({ "title": "Temp" })).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_i64().unwrap();

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
        .clone()
        .oneshot(
            Request::delete(format!("/categories/{other_id}"))
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
            Request::delete(format!("/categories/{id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = app
        .oneshot(
            Request::get(format!("/categories/{id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn search_filters_visible_categories() {
    let (pool, app) = test_state().await;
    let token = login_token(&app).await;
    create_category(&app, &token, json!({ "title": "Prvouka", "description": "animals and plants" })).await;
    create_category(&app, &token, json!({ "title": "Math", "description": "counting to ten" })).await;

    sqlx::query("INSERT INTO users (email, name) VALUES ('other@example.com', 'Other')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO categories (owner_id, title, description, visibility) VALUES
         ((SELECT id FROM users WHERE email = 'other@example.com'), 'Shared animals', 'wildlife', 'public'),
         ((SELECT id FROM users WHERE email = 'other@example.com'), 'Secret animals', 'pets', 'private')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::get("/categories?q=animals")
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
            Request::get("/categories?q=%")
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
async fn categories_require_auth() {
    let (_pool, app) = test_state().await;
    let response = app
        .oneshot(Request::get("/categories").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
