use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Subject {
    pub id: i64,
    pub owner_id: i64,
    pub title: String,
    pub description: String,
    pub language: String,
    pub visibility: String,
    pub status: String,
    pub lesson_count: i64,
    pub tags_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Lesson {
    pub id: i64,
    pub subject_id: i64,
    pub position: i64,
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Question {
    pub id: i64,
    pub lesson_id: i64,
    pub position: i64,
    pub qid: String,
    pub text: String,
    pub qtype: String,
    pub options_json: String,
    pub correct_asset: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Asset {
    pub id: i64,
    pub lesson_id: i64,
    pub asset_key: String,
    pub spec_json: String,
    pub image_blob: Option<Vec<u8>>,
    pub image_mime: Option<String>,
}
