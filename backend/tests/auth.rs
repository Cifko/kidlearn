use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::IntoResponse,
};
use kidlearn_backend::{config::Config, create_app, db, AppState};
use serde_json::json;
use tower::ServiceExt;

async fn test_app() -> axum::Router {
    let config = Config::for_tests();
    let pool = db::init_pool(&config.database_url).await.unwrap();
    create_app(AppState::new(pool, config))
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

async fn mock_tokeninfo(body: serde_json::Value) -> (String, tokio::task::JoinHandle<()>) {
    let app = axum::Router::new().route(
        "/",
        axum::routing::get(move || {
            let body = body.clone();
            async move { axum::Json(body) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (url, handle)
}

async fn google_app(tokeninfo_url: &str) -> axum::Router {
    let mut config = Config::for_tests();
    config.google_client_id = Some("test-client".to_string());
    config.google_tokeninfo_url = tokeninfo_url.to_string();
    let pool = db::init_pool(&config.database_url).await.unwrap();
    create_app(AppState::new(pool, config))
}

async fn google_login(app: &axum::Router) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/auth/google")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "id_token": "test-token" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    (status, json)
}

#[tokio::test]
async fn google_login_creates_user_and_reuses_it() {
    let (url, _server) = mock_tokeninfo(json!({
        "aud": "test-client",
        "sub": "google-123",
        "email": "g@example.com",
        "email_verified": "true",
        "exp": "9999999999",
        "name": "Gus"
    }))
    .await;
    let app = google_app(&url).await;

    let (status, first) = google_login(&app).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["user"]["email"], "g@example.com");
    assert_eq!(first["user"]["name"], "Gus");
    let token = first["token"].as_str().unwrap().to_string();
    assert!(!token.is_empty());

    // Logging in again returns the same user, not a duplicate.
    let (status, second) = google_login(&app).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["user"]["id"], first["user"]["id"]);

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
}

#[tokio::test]
async fn google_login_rejects_wrong_audience() {
    let (url, _server) = mock_tokeninfo(json!({
        "aud": "other-client",
        "sub": "google-123",
        "email": "g@example.com",
        "email_verified": "true",
        "exp": "9999999999"
    }))
    .await;
    let app = google_app(&url).await;
    let (status, _) = google_login(&app).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn google_login_disabled_without_client_id() {
    let app = test_app().await;
    let (status, _) = google_login(&app).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn dev_login_supports_second_user() {
    let app = test_app().await;

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
    let first: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::post("/auth/dev-login")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "who": "dev2" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let second: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(second["user"]["email"], "dev2@localhost");
    assert_ne!(second["user"]["id"], first["user"]["id"]);

    let response = app
        .oneshot(
            Request::get("/me")
                .header(
                    "authorization",
                    format!("Bearer {}", second["token"].as_str().unwrap()),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

async fn mock_graph() -> (String, tokio::task::JoinHandle<()>) {
    let app = axum::Router::new()
        .route(
            "/oauth/access_token",
            axum::routing::get(
                |axum::extract::Query(params): axum::extract::Query<
                    std::collections::HashMap<String, String>,
                >| async move {
                    match params.get("code").map(String::as_str) {
                        Some("good") => axum::Json(json!({
                            "access_token": "tok",
                            "token_type": "bearer",
                        }))
                        .into_response(),
                        Some("noemail") => axum::Json(json!({
                            "access_token": "tok-noemail",
                            "token_type": "bearer",
                        }))
                        .into_response(),
                        // Mirrors a real Graph denial (e.g. redirect_uri
                        // mismatch): HTTP 400 with an OAuthException body.
                        Some("denied") => (
                            StatusCode::BAD_REQUEST,
                            axum::Json(json!({
                                "error": {
                                    "message": "Error validating verification code.",
                                    "type": "OAuthException",
                                    "code": 100,
                                }
                            })),
                        )
                            .into_response(),
                        _ => StatusCode::BAD_REQUEST.into_response(),
                    }
                },
            ),
        )
        .route(
            "/me",
            axum::routing::get(
                |axum::extract::Query(params): axum::extract::Query<
                    std::collections::HashMap<String, String>,
                >| async move {
                    // The app must always send its proof with the token.
                    assert!(params.contains_key("appsecret_proof"));
                    match params.get("access_token").map(String::as_str) {
                        Some("tok") => axum::Json(json!({
                            "id": "fb1",
                            "name": "Fb User",
                            "email": "fb@example.com",
                        }))
                        .into_response(),
                        Some("tok-noemail") => {
                            axum::Json(json!({ "id": "fb2", "name": "No Mail" })).into_response()
                        }
                        _ => StatusCode::UNAUTHORIZED.into_response(),
                    }
                },
            ),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (url, handle)
}

async fn facebook_app(graph_url: &str) -> axum::Router {
    let mut config = Config::for_tests();
    config.facebook_app_id = Some("test-app-id".to_string());
    config.facebook_app_secret = Some("test-app-secret".to_string());
    config.facebook_graph_url = graph_url.to_string();
    let pool = db::init_pool(&config.database_url).await.unwrap();
    create_app(AppState::new(pool, config))
}

async fn facebook_login(app: &axum::Router, code: &str) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/auth/facebook")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "code": code, "redirect_uri": "http://localhost:8081" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}

#[tokio::test]
async fn facebook_login_exchanges_code_for_jwt() {
    let (url, _server) = mock_graph().await;
    let app = facebook_app(&url).await;
    let (status, first) = facebook_login(&app, "good").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["user"]["email"], "fb@example.com");
    assert!(!first["token"].as_str().unwrap().is_empty());

    // Logging in again returns the same user, not a duplicate.
    let (status, second) = facebook_login(&app, "good").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["user"]["id"], first["user"]["id"]);

    let response = app
        .oneshot(
            Request::get("/me")
                .header(
                    "authorization",
                    format!("Bearer {}", first["token"].as_str().unwrap()),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn facebook_login_rejects_bad_code() {
    let (url, _server) = mock_graph().await;
    let app = facebook_app(&url).await;
    let (status, _) = facebook_login(&app, "bogus").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn facebook_login_surfaces_graph_error_detail() {
    let (url, _server) = mock_graph().await;
    let app = facebook_app(&url).await;
    let (status, body) = facebook_login(&app, "denied").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("Error validating verification code"),
        "unexpected body: {body}",
    );
    assert!(
        body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("[redirect_uri sent: http://localhost:8081]"),
        "unexpected body: {body}",
    );
}

#[tokio::test]
async fn facebook_login_requires_email() {
    let (url, _server) = mock_graph().await;
    let app = facebook_app(&url).await;
    let (status, body) = facebook_login(&app, "noemail").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "Facebook account has no email address");
}

#[tokio::test]
async fn facebook_login_disabled_without_config() {
    let app = test_app().await;
    let (status, _) = facebook_login(&app, "good").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}
