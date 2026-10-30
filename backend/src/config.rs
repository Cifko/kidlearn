#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    pub jwt_secret: String,
    pub meta_api_base: String,
    pub meta_api_key: Option<String>,
    pub meta_model: String,
    pub google_client_id: Option<String>,
    pub google_tokeninfo_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        let meta_api_base = env_or("META_API_BASE", "https://api.meta.ai/v1");
        Self {
            port: env_u16("PORT", 3000),
            database_url: env_or("DATABASE_URL", "sqlite://kidlearn.db?mode=rwc"),
            jwt_secret: env_or("JWT_SECRET", "dev-secret-change-me"),
            meta_api_base: meta_api_base.trim_end_matches('/').to_string(),
            meta_api_key: std::env::var("META_API_KEY")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            meta_model: env_or("META_MODEL", "muse-spark-1.3"),
            google_client_id: std::env::var("GOOGLE_CLIENT_ID")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            google_tokeninfo_url: env_or(
                "GOOGLE_TOKENINFO_URL",
                "https://oauth2.googleapis.com/tokeninfo",
            ),
        }
    }

    /// Config for integration tests (in-memory database, no network).
    pub fn for_tests() -> Self {
        Self {
            port: 0,
            database_url: "sqlite::memory:".to_string(),
            jwt_secret: "test-secret".to_string(),
            meta_api_base: "http://localhost:1".to_string(),
            meta_api_key: None,
            meta_model: "test-model".to_string(),
            google_client_id: None,
            google_tokeninfo_url: "http://localhost:1".to_string(),
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_u16(key: &str, default: u16) -> u16 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
