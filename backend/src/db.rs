use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

/// One statement per entry: sqlx does not run multi-statement strings.
const SCHEMA: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        email TEXT UNIQUE NOT NULL,
        name TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
    )",
    "CREATE TABLE IF NOT EXISTS categories (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        owner_id INTEGER NOT NULL REFERENCES users(id),
        title TEXT NOT NULL,
        description TEXT NOT NULL DEFAULT '',
        language TEXT NOT NULL DEFAULT 'en',
        visibility TEXT NOT NULL DEFAULT 'private',
        status TEXT NOT NULL DEFAULT 'ready',
        subcategory_count INTEGER NOT NULL DEFAULT 20,
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
    )",
    "CREATE TABLE IF NOT EXISTS subcategories (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        category_id INTEGER NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
        position INTEGER NOT NULL,
        title TEXT NOT NULL,
        summary TEXT NOT NULL DEFAULT ''
    )",
    "CREATE TABLE IF NOT EXISTS questions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        subcategory_id INTEGER NOT NULL REFERENCES subcategories(id) ON DELETE CASCADE,
        position INTEGER NOT NULL,
        qid TEXT NOT NULL,
        text TEXT NOT NULL,
        qtype TEXT NOT NULL,
        options_json TEXT NOT NULL,
        correct_asset TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS assets (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        subcategory_id INTEGER NOT NULL REFERENCES subcategories(id) ON DELETE CASCADE,
        asset_key TEXT NOT NULL,
        spec_json TEXT NOT NULL,
        svg_text TEXT,
        UNIQUE (subcategory_id, asset_key)
    )",
    "CREATE TABLE IF NOT EXISTS progress (
        user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        question_id INTEGER NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
        correct INTEGER NOT NULL,
        answered_at TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (user_id, question_id)
    )",
];

pub async fn init_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let mut options = SqlitePoolOptions::new();
    if database_url.contains(":memory:") {
        // Each pooled connection would get its own empty database.
        options = options.max_connections(1);
    }
    let pool = options.connect(database_url).await?;
    for statement in SCHEMA {
        sqlx::query(statement).execute(&pool).await?;
    }
    Ok(pool)
}
