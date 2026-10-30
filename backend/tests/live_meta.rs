//! Live smoke test against the real Meta Model API.
//!
//! Runs only when `META_API_KEY` is set (e.g. via `backend/.env`);
//! otherwise it passes trivially. Run with:
//! `set -a; . ./.env; set +a; cargo test --test live_meta`

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use kidlearn_backend::{config::Config, create_app, db, AppState};
use serde_json::json;
use tower::ServiceExt;

fn live_config() -> Option<Config> {
    let from_env = Config::from_env();
    from_env.meta_api_key.as_ref()?;
    Some(Config {
        database_url: "sqlite::memory:".to_string(),
        meta_api_base: from_env.meta_api_base,
        meta_api_key: from_env.meta_api_key,
        meta_model: from_env.meta_model,
        ..Config::for_tests()
    })
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

#[tokio::test]
async fn live_generate_and_render_svg() {
    let Some(config) = live_config() else {
        eprintln!("META_API_KEY not set; skipping live test");
        return;
    };
    let pool = db::init_pool(&config.database_url).await.unwrap();
    let app = create_app(AppState::new(pool.clone(), config));
    let token = login_token(&app).await;

    let response = app
        .clone()
        .oneshot(
            Request::post("/categories")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Colors",
                        "description": "basic colors for kids",
                        "language": "en",
                        "visibility": "private",
                        "subcategory_count": 1
                    })
                    .to_string(),
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

    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/categories/{id}/generate"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "subcategory_count": 1, "min_questions": 2, "max_questions": 3 })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    // Generation runs in the background; poll until it lands.
    let mut detail = serde_json::Value::Null;
    for _ in 0..100 {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        let response = app
            .clone()
            .oneshot(
                Request::get(format!("/categories/{id}"))
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
        detail = serde_json::from_slice(&body).unwrap();
        assert_ne!(detail["status"], "failed");
        if detail["status"] == "ready" {
            break;
        }
    }
    assert_eq!(detail["status"], "ready");
    let subs = detail["subcategories"].as_array().unwrap();
    assert_eq!(subs.len(), 1);
    let question_count = subs[0]["question_count"].as_i64().unwrap();
    assert!(
        (2..=3).contains(&question_count),
        "expected 2-3 questions, got {question_count}"
    );

    let (asset_id,): (i64,) = sqlx::query_as("SELECT id FROM assets LIMIT 1")
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
    let body = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    let svg = String::from_utf8(body.to_vec()).unwrap();
    assert!(
        svg.starts_with("<svg"),
        "SVG did not start with <svg: {}",
        svg.chars().take(80).collect::<String>()
    );
    let (cached,): (Option<String>,) = sqlx::query_as("SELECT svg_text FROM assets WHERE id = ?")
        .bind(asset_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(cached.is_some_and(|s| s.starts_with("<svg")));
}
