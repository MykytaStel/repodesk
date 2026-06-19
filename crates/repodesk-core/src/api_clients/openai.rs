//! OpenAI client implementing [`super::LlmProvider`] via the Chat Completions
//! API. Used for `openai_api` plus legacy `openai`/`chatgpt`/`gpt` completion
//! provider names. The
//! [`super::ThinkingLevel`] hint is not honored by classic chat completions and
//! is intentionally ignored here.

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};

use super::{LlmRequest, LlmResponse, ProviderFuture};

const DEFAULT_BASE_URL: &str = "https://api.openai.com";
const DEFAULT_MODEL: &str = "gpt-4o-mini";

pub struct OpenAiClient {
    api_key: String,
    base_url: String,
    client: Client,
}

impl OpenAiClient {
    pub fn new(api_key: String, base_url: Option<String>) -> Self {
        Self {
            api_key,
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            client: Client::new(),
        }
    }
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    model: String,
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Usage,
}

#[derive(Deserialize)]
struct Choice {
    #[serde(default)]
    message: ChoiceMessage,
}

#[derive(Deserialize, Default)]
struct ChoiceMessage {
    #[serde(default)]
    content: String,
}

#[derive(Deserialize, Default)]
struct Usage {
    #[serde(default)]
    prompt_tokens: usize,
    #[serde(default)]
    completion_tokens: usize,
}

fn build_request(request: &LlmRequest) -> ChatRequest {
    let model = if request.model.trim().is_empty() {
        DEFAULT_MODEL.to_string()
    } else {
        request.model.clone()
    };
    let mut messages = Vec::new();
    if let Some(system) = request.system.as_ref().filter(|s| !s.trim().is_empty()) {
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: system.clone(),
        });
    }
    messages.push(ChatMessage {
        role: "user".to_string(),
        content: request.prompt.clone(),
    });
    ChatRequest {
        model,
        max_tokens: request.max_tokens,
        messages,
    }
}

fn first_text(choices: &[Choice]) -> String {
    choices
        .first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default()
}

impl super::LlmProvider for OpenAiClient {
    fn generate(
        &self,
        prompt: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = RepoDeskResult<String>> + Send>> {
        let fut = self.complete(LlmRequest::new(DEFAULT_MODEL, prompt));
        Box::pin(async move { fut.await.map(|r| r.text) })
    }

    fn is_available(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>> {
        let ok = !self.api_key.trim().is_empty();
        Box::pin(async move { ok })
    }

    fn complete(&self, request: LlmRequest) -> ProviderFuture<RepoDeskResult<LlmResponse>> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let api_key = self.api_key.clone();
        let client = self.client.clone();
        let body = build_request(&request);
        let model = body.model.clone();

        Box::pin(async move {
            let resp = client
                .post(&url)
                .bearer_auth(&api_key)
                .json(&body)
                .send()
                .await
                .map_err(|e| RepoDeskError::ProviderUnavailable {
                    provider: "openai".to_string(),
                    detail: format!("request failed: {e}"),
                })?;

            let status = resp.status();
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let retry_after_secs = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(60);
                return Err(RepoDeskError::ProviderRateLimit {
                    provider: "openai".to_string(),
                    retry_after_secs,
                });
            }
            if !status.is_success() {
                let detail = resp.text().await.unwrap_or_default();
                return Err(RepoDeskError::Api(format!(
                    "OpenAI API error {status}: {detail}"
                )));
            }

            let parsed: ChatResponse = resp
                .json()
                .await
                .map_err(|e| RepoDeskError::Api(format!("failed to parse OpenAI response: {e}")))?;

            Ok(LlmResponse {
                text: first_text(&parsed.choices),
                model: if parsed.model.is_empty() {
                    model
                } else {
                    parsed.model
                },
                input_tokens: parsed.usage.prompt_tokens,
                output_tokens: parsed.usage.completion_tokens,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chat_response() {
        let json = r#"{
            "model": "gpt-4o-mini",
            "choices": [
                {"index": 0, "message": {"role": "assistant", "content": "Hi there"}}
            ],
            "usage": {"prompt_tokens": 10, "completion_tokens": 3, "total_tokens": 13}
        }"#;
        let parsed: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(first_text(&parsed.choices), "Hi there");
        assert_eq!(parsed.usage.prompt_tokens, 10);
        assert_eq!(parsed.usage.completion_tokens, 3);
    }

    #[test]
    fn build_request_prepends_system_message() {
        let req = LlmRequest::new("gpt-x", "question").with_system("you are a bot");
        let body = build_request(&req);
        assert_eq!(body.messages.len(), 2);
        assert_eq!(body.messages[0].role, "system");
        assert_eq!(body.messages[1].role, "user");
    }

    #[test]
    fn build_request_omits_empty_system() {
        let body = build_request(&LlmRequest::new("gpt-x", "q"));
        assert_eq!(body.messages.len(), 1);
        assert_eq!(body.messages[0].role, "user");
    }
}
