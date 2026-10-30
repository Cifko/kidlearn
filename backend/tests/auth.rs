use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use kidlearn_backend::{config::Config, create_app, db, AppState};
use tower::ServiceExt;

async fn test_app() -> axum::Router {
    let config = Config::for_tests();
    let pool = db::init_pool(&config.database_url).await.unwrap();
    create_app(AppState { db: pool, config })
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
    assert_eq!(json["user"]["email"], "dev@localhost");
    json["token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn dev_login_returns_token_and_user() {
    let app = test_app().await;
    let token = login_token(&app).await;
    assert!(!token.is_empty());
}

#[tokio::test]
async fn me_returns_user_with_valid_token() {
    let app = test_app().await;
    let token = login_token(&app).await;
    let response = app
        .oneshot(
            Request::get("/me")
                .header("authorization", format!("Bearer {token}"))
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
    assert_eq!(json["email"], "dev@localhost");
}

#[tokio::test]
async fn me_rejects_missing_token() {
    let app = test_app().await;
    let response = app
        .oneshot(Request::get("/me").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn me_rejects_bad_token() {
    let app = test_app().await;
    let response = app
        .oneshot(
            Request::get("/me")
                .header("authorization", "Bearer nope")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn me_rejects_token_for_unknown_user() {
    // Same secret, different database: the token is valid but the user
    // does not exist here.
    let app_a = test_app().await;
    let token = login_token(&app_a).await;
    let app_b = test_app().await;
    let response = app_b
        .oneshot(
            Request::get("/me")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
