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
    pub grade: Option<i64>,
    pub modality: String,
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

/// A deterministic function plot, drawn by the app with no AI call.
/// Expressions use explicit operators (`2*x`, never `2x`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotFunction {
    #[serde(default)]
    pub expr: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotPoint {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotSpec {
    #[serde(default)]
    pub functions: Vec<PlotFunction>,
    #[serde(default)]
    pub x_min: f64,
    #[serde(default)]
    pub x_max: f64,
    #[serde(default)]
    pub y_min: f64,
    #[serde(default)]
    pub y_max: f64,
    #[serde(default)]
    pub points: Vec<PlotPoint>,
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
    pub options_text_json: String,
    pub correct_text: String,
    pub figure_asset: String,
    pub plot_json: String,
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
