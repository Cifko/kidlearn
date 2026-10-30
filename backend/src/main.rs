use kidlearn_backend::{config::Config, create_app, db, AppState};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let config = Config::from_env();
    let db = db::init_pool(&config.database_url)
        .await
        .expect("database init failed");
    let app = create_app(AppState {
        db,
        config: config.clone(),
    });
    let addr = format!("127.0.0.1:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind failed");
    println!("kidlearn backend listening on http://{addr}");
    axum::serve(listener, app).await.expect("serve failed");
}
