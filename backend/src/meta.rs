use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{config::Config, error::AppError};

/// Client for the Meta Model API at `api.meta.ai` (OpenAI-compatible `/chat/completions`).
#[derive(Clone)]
pub struct MetaClient {
    base_url: String,
    api_key: String,
    model: String,
    http: reqwest::Client,
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
        })
    }

    /// Asks the model for a JSON document parsed into `T`.
    pub async fn chat_json<T: DeserializeOwned>(
        &self,
        prompt: &str,
        temperature: f32,
    ) -> Result<T, AppError> {
        let text = self.chat(prompt, temperature, true).await?;
        serde_json::from_str(extract_json(&text)).map_err(|err| {
            AppError::ServiceUnavailable(format!(
                "generation backend returned invalid JSON: {err}"
            ))
        })
    }

    /// Asks the model for raw text (used for SVG art).
    pub async fn chat_text(&self, prompt: &str) -> Result<String, AppError> {
        self.chat(prompt, 0.2, false).await
    }

    async fn chat(
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
                AppError::ServiceUnavailable(format!("generation backend unreachable: {err}"))
            })?;
        if !response.status().is_success() {
            let status = response.status();
            let detail = response.text().await.unwrap_or_default();
            let detail = detail.chars().take(300).collect::<String>();
            return Err(AppError::ServiceUnavailable(format!(
                "generation backend error {status}: {detail}"
            )));
        }
        let parsed: ChatResponse = response.json().await.map_err(|err| {
            AppError::ServiceUnavailable(format!("generation backend sent a bad envelope: {err}"))
        })?;
        parsed
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .ok_or_else(|| {
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
