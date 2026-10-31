use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;

/// One statement per entry: sqlx does not run multi-statement strings.
const SCHEMA: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        email TEXT UNIQUE NOT NULL,
        name TEXT NOT NULL,
        google_sub TEXT,
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
    )",
    "CREATE TABLE IF NOT EXISTS subjects (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        owner_id INTEGER NOT NULL REFERENCES users(id),
        title TEXT NOT NULL,
        description TEXT NOT NULL DEFAULT '',
        language TEXT NOT NULL DEFAULT 'en',
        visibility TEXT NOT NULL DEFAULT 'private',
        status TEXT NOT NULL DEFAULT 'ready',
        lesson_count INTEGER NOT NULL DEFAULT 20,
        tags_json TEXT NOT NULL DEFAULT '[]',
        generated_lessons INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
    )",
    "CREATE TABLE IF NOT EXISTS lessons (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        subject_id INTEGER NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
        position INTEGER NOT NULL,
        title TEXT NOT NULL,
        summary TEXT NOT NULL DEFAULT ''
    )",
    "CREATE TABLE IF NOT EXISTS questions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        lesson_id INTEGER NOT NULL REFERENCES lessons(id) ON DELETE CASCADE,
        position INTEGER NOT NULL,
        qid TEXT NOT NULL,
        text TEXT NOT NULL,
        qtype TEXT NOT NULL,
        options_json TEXT NOT NULL,
        correct_asset TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS assets (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        lesson_id INTEGER NOT NULL REFERENCES lessons(id) ON DELETE CASCADE,
        asset_key TEXT NOT NULL,
        spec_json TEXT NOT NULL,
        svg_text TEXT,
        UNIQUE (lesson_id, asset_key)
    )",
    "CREATE TABLE IF NOT EXISTS progress (
        user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        question_id INTEGER NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
        correct INTEGER NOT NULL,
        answered_at TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (user_id, question_id)
    )",
    "CREATE TABLE IF NOT EXISTS stats (
        user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
        xp INTEGER NOT NULL DEFAULT 0,
        streak_days INTEGER NOT NULL DEFAULT 0,
        last_active_date TEXT
    )",
    "CREATE TABLE IF NOT EXISTS likes (
        user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        subject_id INTEGER NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (user_id, subject_id)
    )",
];

/// ALTERs for databases created before the matching column existed.
/// Each runs on boot; already-applied ones are ignored.
const MIGRATIONS: &[&str] = &[
    "ALTER TABLE users ADD COLUMN google_sub TEXT",
    "ALTER TABLE categories RENAME TO subjects",
    "ALTER TABLE subcategories RENAME TO lessons",
    "ALTER TABLE lessons RENAME COLUMN category_id TO subject_id",
    "ALTER TABLE questions RENAME COLUMN subcategory_id TO lesson_id",
    "ALTER TABLE assets RENAME COLUMN subcategory_id TO lesson_id",
    "ALTER TABLE subjects ADD COLUMN tags_json TEXT NOT NULL DEFAULT '[]'",
    "ALTER TABLE subjects ADD COLUMN generated_lessons INTEGER NOT NULL DEFAULT 0",
];

pub async fn init_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let mut options = SqlitePoolOptions::new();
    if database_url.contains(":memory:") {
        // Each pooled connection would get its own empty database.
        options = options.max_connections(1);
    }
    // WAL + a busy timeout: parallel generation tasks share the pool and
    // SQLite otherwise fails fast with "database is locked".
    let connect = SqliteConnectOptions::from_str(database_url)?
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(10));
    let pool = options.connect_with(connect).await?;
    for statement in SCHEMA {
        sqlx::query(statement).execute(&pool).await?;
    }
    for migration in MIGRATIONS {
        match sqlx::query(migration).execute(&pool).await {
            Ok(_) => {}
            Err(err)
                if err.to_string().contains("duplicate column")
                    || err.to_string().contains("no such table")
                    || err.to_string().contains("no such column") => {}
            Err(err) => return Err(err),
        }
    }
    Ok(pool)
}
