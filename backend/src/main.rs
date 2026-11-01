use kidlearn_backend::{config::Config, create_app, db, AppState};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    dotenvy::dotenv().ok();
    let config = Config::from_env();
    let db = db::init_pool(&config.database_url)
        .await
        .expect("database init failed");
    tracing::info!("database ready");
    let app = create_app(AppState::new(db, config.clone()));
    // All interfaces: a physical phone reaches the backend via the LAN IP.
    // Localhost keeps working as before.
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind failed");
    tracing::info!("kidlearn backend listening on http://{addr}");
    axum::serve(listener, app).await.expect("serve failed");
}
