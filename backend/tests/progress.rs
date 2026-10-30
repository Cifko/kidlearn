use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use kidlearn_backend::{config::Config, create_app, db, AppState};
use serde_json::json;
use tower::ServiceExt;

struct Seed {
    pool: sqlx::SqlitePool,
    app: axum::Router,
    token: String,
    category_id: i64,
    sub1: i64,
    sub2: i64,
    questions: Vec<i64>,
}

async fn seed() -> Seed {
    let config = Config::for_tests();
    let pool = db::init_pool(&config.database_url).await.unwrap();
    let app = create_app(AppState::new(pool.clone(), config));

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
    let token = json["token"].as_str().unwrap().to_string();

    let (category_id,): (i64,) = sqlx::query_as(
        "INSERT INTO categories (owner_id, title, visibility)
         VALUES ((SELECT id FROM users WHERE email = 'dev@localhost'), 'Math', 'private')
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (sub1,): (i64,) = sqlx::query_as(
        "INSERT INTO subcategories (category_id, position, title) VALUES (?, 1, 'Counting') RETURNING id",
    )
    .bind(category_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (sub2,): (i64,) = sqlx::query_as(
        "INSERT INTO subcategories (category_id, position, title) VALUES (?, 2, 'Adding') RETURNING id",
    )
    .bind(category_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    for key in ["red_ball", "blue_cube", "green_cone"] {
        sqlx::query("INSERT INTO assets (subcategory_id, asset_key, spec_json) VALUES (?, ?, '{}')")
            .bind(sub1)
            .bind(key)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO assets (subcategory_id, asset_key, spec_json) VALUES (?, 'yellow_star', '{}')")
        .bind(sub2)
        .execute(&pool)
        .await
        .unwrap();

    let mut questions = Vec::new();
    for (index, correct) in ["red_ball", "blue_cube", "green_cone"].into_iter().enumerate() {
        let position = index as i64 + 1;
        let (id,): (i64,) = sqlx::query_as(
            "INSERT INTO questions (subcategory_id, position, qid, text, qtype, options_json, correct_asset)
             VALUES (?, ?, ?, 'Pick it', 'multiple_choice_image', '[\"red_ball\",\"blue_cube\",\"green_cone\"]', ?)
             RETURNING id",
        )
        .bind(sub1)
        .bind(position)
        .bind(format!("q{position}"))
        .bind(correct)
        .fetch_one(&pool)
        .await
        .unwrap();
        questions.push(id);
    }

    Seed {
        pool,
        app,
        token,
        category_id,
        sub1,
        sub2,
        questions,
    }
}

async fn get_json(app: &axum::Router, token: &str, path: String) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::get(path)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or(json!({}));
    (status, json)
}

async fn answer(
    app: &axum::Router,
    token: &str,
    sub_id: i64,
    question_id: i64,
    asset_key: &str,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/subcategories/{sub_id}/answer"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "question_id": question_id, "asset_key": asset_key }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or(json!({}));
    (status, json)
}

#[tokio::test]
async fn lesson_returns_questions_without_answers() {
    let seed = seed().await;
    let (status, lesson) = get_json(
        &seed.app,
        &seed.token,
        format!("/subcategories/{}/lesson", seed.sub1),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(lesson["subcategory"]["title"], "Counting");
    let questions = lesson["questions"].as_array().unwrap();
    assert_eq!(questions.len(), 3);
    for question in questions {
        assert!(question.get("correct_asset").is_none());
        assert!(question.get("correct").is_none());
        let options = question["options"].as_array().unwrap();
        assert_eq!(options.len(), 3);
        assert_eq!(options[0]["asset_key"], "red_ball");
        assert!(options[0]["svg_url"]
            .as_str()
            .unwrap()
            .starts_with("/assets/"));
        assert!(options[0]["svg_url"].as_str().unwrap().ends_with("/svg"));
    }
}

#[tokio::test]
async fn second_lesson_locks_until_first_is_attempted() {
    let seed = seed().await;
    let (status, _) = get_json(
        &seed.app,
        &seed.token,
        format!("/subcategories/{}/lesson", seed.sub2),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Answering a locked lesson is rejected too.
    let (sub2_question,): (i64,) = sqlx::query_as(
        "INSERT INTO questions (subcategory_id, position, qid, text, qtype, options_json, correct_asset)
         VALUES (?, 1, 'q1', 'Pick it', 'multiple_choice_image', '[\"yellow_star\"]', 'yellow_star')
         RETURNING id",
    )
    .bind(seed.sub2)
    .fetch_one(&seed.pool)
    .await
    .unwrap();
    let (status, _) = answer(&seed.app, &seed.token, seed.sub2, sub2_question, "yellow_star").await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    for (index, question_id) in seed.questions.iter().enumerate() {
        let key = ["red_ball", "blue_cube", "green_cone"][index];
        let (status, result) = answer(&seed.app, &seed.token, seed.sub1, *question_id, key).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(result["correct"], true);
    }

    let (status, _) = get_json(
        &seed.app,
        &seed.token,
        format!("/subcategories/{}/lesson", seed.sub2),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn wrong_answer_reveals_correct_key_and_records_progress() {
    let seed = seed().await;
    let (status, result) = answer(&seed.app, &seed.token, seed.sub1, seed.questions[0], "blue_cube").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["correct"], false);
    assert_eq!(result["correct_asset_key"], "red_ball");

    let (status, progress) = get_json(
        &seed.app,
        &seed.token,
        format!("/categories/{}/progress", seed.category_id),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let subs = progress["subcategories"].as_array().unwrap();
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0]["answered"], 1);
    assert_eq!(subs[0]["correct"], 0);
    assert_eq!(subs[0]["total"], 3);
    assert_eq!(subs[0]["unlocked"], true);
    assert_eq!(subs[1]["unlocked"], false);

    // Re-answering overwrites the attempt.
    let (status, result) = answer(&seed.app, &seed.token, seed.sub1, seed.questions[0], "red_ball").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["correct"], true);
    let (_, progress) = get_json(
        &seed.app,
        &seed.token,
        format!("/categories/{}/progress", seed.category_id),
    )
    .await;
    assert_eq!(progress["subcategories"][0]["correct"], 1);
}

#[tokio::test]
async fn lesson_requires_auth_and_respects_visibility() {
    let seed = seed().await;
    let response = seed
        .app
        .clone()
        .oneshot(
            Request::get(format!("/subcategories/{}/lesson", seed.sub1))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    sqlx::query("INSERT INTO users (email, name) VALUES ('other@example.com', 'Other')")
        .execute(&seed.pool)
        .await
        .unwrap();
    let (other_cat,): (i64,) = sqlx::query_as(
        "INSERT INTO categories (owner_id, title, visibility)
         VALUES ((SELECT id FROM users WHERE email = 'other@example.com'), 'Theirs', 'private')
         RETURNING id",
    )
    .fetch_one(&seed.pool)
    .await
    .unwrap();
    let (other_sub,): (i64,) = sqlx::query_as(
        "INSERT INTO subcategories (category_id, position, title) VALUES (?, 1, 'Secret') RETURNING id",
    )
    .bind(other_cat)
    .fetch_one(&seed.pool)
    .await
    .unwrap();
    let (status, _) = get_json(
        &seed.app,
        &seed.token,
        format!("/subcategories/{other_sub}/lesson"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn xp_awarded_once_per_question() {
    let seed = seed().await;

    let (status, result) =
        answer(&seed.app, &seed.token, seed.sub1, seed.questions[0], "red_ball").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["xp_earned"], 10);
    assert_eq!(result["total_xp"], 10);

    // Re-answering correctly earns nothing.
    let (_, result) =
        answer(&seed.app, &seed.token, seed.sub1, seed.questions[0], "red_ball").await;
    assert_eq!(result["xp_earned"], 0);
    assert_eq!(result["total_xp"], 10);

    // Wrong first, then correct: XP on the first correct attempt.
    let (_, result) =
        answer(&seed.app, &seed.token, seed.sub1, seed.questions[1], "red_ball").await;
    assert_eq!(result["correct"], false);
    assert_eq!(result["xp_earned"], 0);
    let (_, result) =
        answer(&seed.app, &seed.token, seed.sub1, seed.questions[1], "blue_cube").await;
    assert_eq!(result["xp_earned"], 10);
    assert_eq!(result["total_xp"], 20);

    let (status, stats) = get_json(&seed.app, &seed.token, "/stats".to_string()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stats["xp"], 20);
    assert_eq!(stats["streak_days"], 1);
}

#[tokio::test]
async fn streak_extends_on_consecutive_days_and_resets_after_gap() {
    let seed = seed().await;
    answer(&seed.app, &seed.token, seed.sub1, seed.questions[0], "red_ball").await;

    // Pretend the last activity was yesterday with a 4-day streak.
    sqlx::query(
        "UPDATE stats SET last_active_date = date('now', '-1 day'), streak_days = 4
         WHERE user_id = (SELECT id FROM users WHERE email = 'dev@localhost')",
    )
    .execute(&seed.pool)
    .await
    .unwrap();
    answer(&seed.app, &seed.token, seed.sub1, seed.questions[1], "blue_cube").await;
    let (_, stats) = get_json(&seed.app, &seed.token, "/stats".to_string()).await;
    assert_eq!(stats["streak_days"], 5);

    // A 5-day gap restarts the streak at one.
    sqlx::query(
        "UPDATE stats SET last_active_date = date('now', '-5 days')
         WHERE user_id = (SELECT id FROM users WHERE email = 'dev@localhost')",
    )
    .execute(&seed.pool)
    .await
    .unwrap();
    answer(&seed.app, &seed.token, seed.sub1, seed.questions[2], "green_cone").await;
    let (_, stats) = get_json(&seed.app, &seed.token, "/stats".to_string()).await;
    assert_eq!(stats["streak_days"], 1);
}
