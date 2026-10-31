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

    /// Asks the model for raw text (used for SVG art).
    pub async fn chat_text(&self, ctx: &CallContext, prompt: &str) -> Result<String, AppError> {
        self.chat(ctx, prompt, 0.2, false).await
    }

    async fn record(
        &self,
        ctx: &CallContext,
        prompt: &str,
        result: &Result<String, AppError>,
        duration_ms: i64,
    ) {
        let Some(recorder) = &self.recorder else {
            return;
        };
        let (response, error) = match result {
            Ok(text) => (Some(text.as_str()), None),
            Err(err) => (None, Some(format!("{err:?}"))),
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
        self.record(ctx, prompt, &result, start.elapsed().as_millis() as i64)
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
}
