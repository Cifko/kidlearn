use base64::Engine as _;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{config::Config, error::AppError};

/// What one model call was for. Stored alongside the prompt and answer so
/// bad outputs (like a backwards ladybug) can be traced back to the input.
#[derive(Debug, Clone, Default)]
pub struct CallContext {
    pub kind: &'static str,
    pub subject_id: Option<i64>,
    pub lesson_id: Option<i64>,
    pub asset_id: Option<i64>,
    pub label: String,
}

/// Client for the Meta Model API at `api.meta.ai` (OpenAI-compatible `/chat/completions`).
#[derive(Clone)]
pub struct MetaClient {
    base_url: String,
    api_key: String,
    model: String,
    image_model: String,
    http: reqwest::Client,
    recorder: Option<Recorder>,
}

#[derive(Clone)]
struct Recorder {
    pool: sqlx::SqlitePool,
    user_id: i64,
}

impl MetaClient {
    /// Returns `None` when no API key is configured, so the server can run
    /// (without generation) before the key is installed.
    pub fn from_config(config: &Config) -> Option<Self> {
        let api_key = config.meta_api_key.clone()?;
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .ok()?;
        Some(Self {
            base_url: config.meta_api_base.clone(),
            api_key,
            model: config.meta_model.clone(),
            image_model: config.meta_image_model.clone(),
            http,
            recorder: None,
        })
    }

    /// Stores every prompt and answer in `llm_calls` for later inspection.
    pub fn with_recorder(mut self, pool: sqlx::SqlitePool, user_id: i64) -> Self {
        self.recorder = Some(Recorder { pool, user_id });
        self
    }

    /// Asks the model for a JSON document parsed into `T`.
    pub async fn chat_json<T: DeserializeOwned>(
        &self,
        ctx: &CallContext,
        prompt: &str,
        temperature: f32,
    ) -> Result<T, AppError> {
        let text = self.chat(ctx, prompt, temperature, true).await?;
        serde_json::from_str(extract_json(&text)).map_err(|err| {
            tracing::error!(%err, "generation backend returned invalid JSON");
            AppError::ServiceUnavailable(format!("generation backend returned invalid JSON: {err}"))
        })
    }

    /// Asks the image model for one picture. Returns the raw bytes plus
    /// a detected MIME type. Only the prompt and a byte-count summary are
    /// recorded; the pixels stay out of `llm_calls`.
    pub async fn generate_image(
        &self,
        ctx: &CallContext,
        prompt: &str,
    ) -> Result<(Vec<u8>, String), AppError> {
        let start = std::time::Instant::now();
        let result = self.images_inner(prompt).await;
        let (response, error) = match &result {
            Ok((bytes, mime)) => (Some(format!("[{mime}, {} bytes]", bytes.len())), None),
            Err(err) => (None, Some(format!("{err:?}"))),
        };
        self.record(
            ctx,
            prompt,
            response.as_deref(),
            error,
            start.elapsed().as_millis() as i64,
        )
        .await;
        result
    }

    async fn record(
        &self,
        ctx: &CallContext,
        prompt: &str,
        response: Option<&str>,
        error: Option<String>,
        duration_ms: i64,
    ) {
        let Some(recorder) = &self.recorder else {
            return;
        };
        if let Err(err) = sqlx::query(
            "INSERT INTO llm_calls (user_id, kind, subject_id, lesson_id, asset_id, label,
                 model, prompt, response, error, duration_ms)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(recorder.user_id)
        .bind(ctx.kind)
        .bind(ctx.subject_id)
        .bind(ctx.lesson_id)
        .bind(ctx.asset_id)
        .bind(&ctx.label)
        .bind(&self.model)
        .bind(prompt)
        .bind(response)
        .bind(error)
        .bind(duration_ms)
        .execute(&recorder.pool)
        .await
        {
            tracing::warn!(%err, "failed to record llm call");
        }
    }

    async fn chat(
        &self,
        ctx: &CallContext,
        prompt: &str,
        temperature: f32,
        json_mode: bool,
    ) -> Result<String, AppError> {
        let start = std::time::Instant::now();
        let result = self.chat_inner(prompt, temperature, json_mode).await;
        let (response, error) = match &result {
            Ok(text) => (Some(text.as_str()), None),
            Err(err) => (None, Some(format!("{err:?}"))),
        };
        self.record(
            ctx,
            prompt,
            response,
            error,
            start.elapsed().as_millis() as i64,
        )
        .await;
        result
    }

    async fn chat_inner(
        &self,
        prompt: &str,
        temperature: f32,
        json_mode: bool,
    ) -> Result<String, AppError> {
        #[derive(Serialize)]
        struct Message<'a> {
            role: &'a str,
            content: &'a str,
        }
        #[derive(Serialize)]
        struct ResponseFormat {
            #[serde(rename = "type")]
            kind: &'static str,
        }
        #[derive(Serialize)]
        struct ChatRequest<'a> {
            model: &'a str,
            messages: Vec<Message<'a>>,
            temperature: f32,
            #[serde(skip_serializing_if = "Option::is_none")]
            response_format: Option<ResponseFormat>,
        }
        #[derive(Deserialize)]
        struct ChatResponse {
            choices: Vec<Choice>,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: MessageContent,
        }
        #[derive(Deserialize)]
        struct MessageContent {
            content: String,
        }

        let body = ChatRequest {
            model: &self.model,
            messages: vec![
                Message {
                    role: "system",
                    content: "Follow the task specification exactly. Return only the requested output format.",
                },
                Message {
                    role: "user",
                    content: prompt,
                },
            ],
            temperature,
            response_format: json_mode.then_some(ResponseFormat { kind: "json_object" }),
        };

        let url = format!("{}/chat/completions", self.base_url);
        let response = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|err| {
                tracing::error!(%err, "generation backend unreachable");
                AppError::ServiceUnavailable(format!("generation backend unreachable: {err}"))
            })?;
        if !response.status().is_success() {
            let status = response.status();
            let detail = response.text().await.unwrap_or_default();
            let detail = detail.chars().take(300).collect::<String>();
            tracing::warn!(%status, %detail, "generation backend error");
            return Err(AppError::ServiceUnavailable(format!(
                "generation backend error {status}: {detail}"
            )));
        }
        let parsed: ChatResponse = response.json().await.map_err(|err| {
            tracing::error!(%err, "generation backend sent a bad envelope");
            AppError::ServiceUnavailable(format!("generation backend sent a bad envelope: {err}"))
        })?;
        parsed
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .ok_or_else(|| {
                tracing::error!("generation backend returned no choices");
                AppError::ServiceUnavailable("generation backend returned no choices".to_string())
            })
    }

    async fn images_inner(&self, prompt: &str) -> Result<(Vec<u8>, String), AppError> {
        #[derive(Serialize)]
        struct ImagesRequest<'a> {
            model: &'a str,
            prompt: &'a str,
            size: &'static str,
        }
        #[derive(Deserialize)]
        struct ImagesResponse {
            data: Vec<ImageData>,
        }
        #[derive(Deserialize)]
        struct ImageData {
            b64_json: String,
        }

        let body = ImagesRequest {
            model: &self.image_model,
            prompt,
            size: "512x512",
        };
        let url = format!("{}/images/generations", self.base_url);
        let response = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|err| {
                tracing::error!(%err, "image backend unreachable");
                AppError::ServiceUnavailable(format!("image backend unreachable: {err}"))
            })?;
        if !response.status().is_success() {
            let status = response.status();
            let detail = response.text().await.unwrap_or_default();
            let detail = detail.chars().take(300).collect::<String>();
            tracing::warn!(%status, %detail, "image backend error");
            return Err(AppError::ServiceUnavailable(format!(
                "image backend error {status}: {detail}"
            )));
        }
        let parsed: ImagesResponse = response.json().await.map_err(|err| {
            tracing::error!(%err, "image backend sent a bad envelope");
            AppError::ServiceUnavailable(format!("image backend sent a bad envelope: {err}"))
        })?;
        let b64 = parsed
            .data
            .into_iter()
            .next()
            .map(|image| image.b64_json)
            .ok_or_else(|| {
                tracing::error!("image backend returned no images");
                AppError::ServiceUnavailable("image backend returned no images".to_string())
            })?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .map_err(|err| {
                tracing::error!(%err, "image backend returned invalid base64");
                AppError::ServiceUnavailable(format!(
                    "image backend returned invalid base64: {err}"
                ))
            })?;
        let mime = detect_image_mime(&bytes).ok_or_else(|| {
            tracing::error!("image backend returned an unknown image format");
            AppError::ServiceUnavailable(
                "image backend returned an unknown image format".to_string(),
            )
        })?;
        Ok((bytes, mime.to_string()))
    }
}

/// Sniffs webp/png/jpeg/gif from magic bytes; the images endpoint does
/// not report a content type.
fn detect_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF8") {
        Some("image/gif")
    } else {
        None
    }
}

/// Strips markdown code fences some models add around JSON.
fn extract_json(text: &str) -> &str {
    let trimmed = text.trim();
    let without_open = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed);
    without_open
        .strip_suffix("```")
        .unwrap_or(without_open)
        .trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_handles_fences_and_plain_json() {
        assert_eq!(extract_json("{\"a\":1}"), "{\"a\":1}");
        assert_eq!(extract_json("  \n{\"a\":1}\n "), "{\"a\":1}");
        assert_eq!(extract_json("```json\n{\"a\":1}\n```"), "{\"a\":1}");
        assert_eq!(extract_json("```\n{\"a\":1}\n```"), "{\"a\":1}");
    }

    #[test]
    fn client_is_none_without_api_key() {
        let config = Config::for_tests();
        assert!(config.meta_api_key.is_none());
        assert!(MetaClient::from_config(&config).is_none());
    }

    #[test]
    fn detect_image_mime_sniffs_magic_bytes() {
        assert_eq!(
            detect_image_mime(b"RIFF\x00\x00\x00\x00WEBP"),
            Some("image/webp")
        );
        assert_eq!(
            detect_image_mime(&[0x89, b'P', b'N', b'G', 0x0d]),
            Some("image/png")
        );
        assert_eq!(
            detect_image_mime(&[0xFF, 0xD8, 0xFF, 0xE0]),
            Some("image/jpeg")
        );
        assert_eq!(detect_image_mime(b"GIF89a"), Some("image/gif"));
        assert_eq!(detect_image_mime(b"<svg"), None);
        assert_eq!(detect_image_mime(b""), None);
    }
}
