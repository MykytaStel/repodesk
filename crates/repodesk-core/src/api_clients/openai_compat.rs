//! Generic **OpenAI-compatible** client over the Chat Completions API
//! (`/v1/chat/completions`). This is the lingua franca most local servers and
//! third-party vendors speak — LM Studio, DeepSeek, Groq, OpenRouter, Together,
//! Mistral, xAI — so one client (pointed at a configurable `base_url`) covers all
//! of them. (OpenAI's own first-party client uses the newer Responses API; see
//! [`super::openai::OpenAiClient`].)

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};

use super::{LlmRequest, LlmResponse, ProviderFuture};

pub struct OpenAiCompatClient {
    api_key: String,
    base_url: String,
    default_model: String,
    /// Provider id used in error messages (e.g. `lm_studio`, `deepseek`).
    label: String,
    client: Client,
}

impl OpenAiCompatClient {
    pub fn new(
        api_key: String,
        base_url: String,
        default_model: String,
        label: impl Into<String>,
    ) -> Self {
        Self {
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            default_model,
            label: label.into(),
            client: Client::new(),
        }
    }
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize, Default)]
struct ChatResponse {
    #[serde(default)]
    model: String,
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    usage: ChatUsage,
}

#[derive(Deserialize, Default)]
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
struct ChatUsage {
    #[serde(default)]
    prompt_tokens: usize,
    #[serde(default)]
    completion_tokens: usize,
}

fn build_request(request: &LlmRequest, default_model: &str) -> ChatRequest {
    let model = if request.model.trim().is_empty() {
        default_model.to_string()
    } else {
        request.model.clone()
    };
    let mut messages = Vec::new();
    if let Some(system) = request
        .system
        .as_ref()
        .filter(|system| !system.trim().is_empty())
    {
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
        messages,
        max_tokens: request.max_tokens,
    }
}

fn extract_text(response: &ChatResponse) -> String {
    response
        .choices
        .first()
        .map(|choice| choice.message.content.clone())
        .unwrap_or_default()
}

impl super::LlmProvider for OpenAiCompatClient {
    fn generate(
        &self,
        prompt: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = RepoDeskResult<String>> + Send>> {
        let fut = self.complete(LlmRequest::new(self.default_model.clone(), prompt));
        Box::pin(async move { fut.await.map(|r| r.text) })
    }

    fn is_available(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>> {
        // Local servers ignore auth; remote ones need a key. Either way a
        // configured client is offered, and a real failure surfaces on complete().
        Box::pin(async move { true })
    }

    fn complete(&self, request: LlmRequest) -> ProviderFuture<RepoDeskResult<LlmResponse>> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let api_key = self.api_key.clone();
        let client = self.client.clone();
        let label = self.label.clone();
        let body = build_request(&request, &self.default_model);
        let model = body.model.clone();

        Box::pin(async move {
            let mut builder = client.post(&url).json(&body);
            // Send auth only when a key is configured (local servers need none).
            if !api_key.trim().is_empty() {
                builder = builder.bearer_auth(&api_key);
            }
            let resp = builder
                .send()
                .await
                .map_err(|e| RepoDeskError::ProviderUnavailable {
                    provider: label.clone(),
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
                    provider: label,
                    retry_after_secs,
                });
            }
            if !status.is_success() {
                let detail = resp.text().await.unwrap_or_default();
                return Err(RepoDeskError::Api(format!(
                    "{label} API error {status}: {detail}"
                )));
            }

            let parsed: ChatResponse = resp.json().await.map_err(|e| {
                RepoDeskError::Api(format!("failed to parse {label} response: {e}"))
            })?;

            Ok(LlmResponse {
                text: extract_text(&parsed),
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
    fn build_request_includes_system_then_user() {
        let req = LlmRequest::new("deepseek-chat", "hello").with_system("be terse");
        let body = build_request(&req, "fallback");
        assert_eq!(body.model, "deepseek-chat");
        assert_eq!(body.messages.len(), 2);
        assert_eq!(body.messages[0].role, "system");
        assert_eq!(body.messages[1].role, "user");
        assert_eq!(body.messages[1].content, "hello");
    }

    #[test]
    fn build_request_falls_back_to_default_model() {
        let body = build_request(&LlmRequest::new("", "q"), "local-model");
        assert_eq!(body.model, "local-model");
        assert_eq!(body.messages.len(), 1);
    }

    #[test]
    fn parses_chat_completion() {
        let json = r#"{
            "model": "deepseek-chat",
            "choices": [{"message": {"role": "assistant", "content": "Hi there"}}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 3}
        }"#;
        let parsed: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(extract_text(&parsed), "Hi there");
        assert_eq!(parsed.usage.prompt_tokens, 10);
        assert_eq!(parsed.usage.completion_tokens, 3);
    }
}
