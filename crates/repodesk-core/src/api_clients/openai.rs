//! OpenAI client implementing [`super::LlmProvider`] via the **Responses API**
//! (`/v1/responses`). Used for `openai_api` plus legacy `openai`/`chatgpt`/`gpt`
//! completion provider names.
//!
//! The Responses API replaces Chat Completions for new work: the prompt is the
//! `input`, the system prompt is `instructions`, the output cap is
//! `max_output_tokens`, and the [`super::ThinkingLevel`] maps to `reasoning.effort`
//! (sent only when non-`None`, since plain chat models reject the field). A direct
//! Responses API call is **not** the same as running Codex CLI — it is a bounded
//! request/response, not a repository-editing agent.

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
struct ResponsesRequest {
    model: String,
    input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    max_output_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<Reasoning>,
}

#[derive(Serialize)]
struct Reasoning {
    effort: String,
}

#[derive(Deserialize)]
struct ResponsesResponse {
    #[serde(default)]
    model: String,
    /// Convenience aggregate some responses include; preferred when present.
    #[serde(default)]
    output_text: Option<String>,
    #[serde(default)]
    output: Vec<OutputItem>,
    #[serde(default)]
    usage: Usage,
}

#[derive(Deserialize, Default)]
struct OutputItem {
    #[serde(default)]
    content: Vec<ContentPart>,
}

#[derive(Deserialize, Default)]
struct ContentPart {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

#[derive(Deserialize, Default)]
struct Usage {
    #[serde(default)]
    input_tokens: usize,
    #[serde(default)]
    output_tokens: usize,
}

fn build_request(request: &LlmRequest) -> ResponsesRequest {
    let model = if request.model.trim().is_empty() {
        DEFAULT_MODEL.to_string()
    } else {
        request.model.clone()
    };
    let instructions = request
        .system
        .as_ref()
        .filter(|system| !system.trim().is_empty())
        .cloned();
    let reasoning = request.thinking.reasoning_effort().map(|effort| Reasoning {
        effort: effort.to_string(),
    });
    ResponsesRequest {
        model,
        input: request.prompt.clone(),
        instructions,
        max_output_tokens: request.max_tokens,
        reasoning,
    }
}

/// Concatenate every `output_text` content part across the response's output
/// items, preferring the convenience `output_text` aggregate when present.
fn extract_text(response: &ResponsesResponse) -> String {
    if let Some(text) = response.output_text.as_ref().filter(|t| !t.is_empty()) {
        return text.clone();
    }
    let mut out = String::new();
    for item in &response.output {
        for part in &item.content {
            if part.kind == "output_text" {
                out.push_str(&part.text);
            }
        }
    }
    out
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
        let url = format!("{}/v1/responses", self.base_url);
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

            let parsed: ResponsesResponse = resp
                .json()
                .await
                .map_err(|e| RepoDeskError::Api(format!("failed to parse OpenAI response: {e}")))?;

            Ok(LlmResponse {
                text: extract_text(&parsed),
                model: if parsed.model.is_empty() {
                    model
                } else {
                    parsed.model
                },
                input_tokens: parsed.usage.input_tokens,
                output_tokens: parsed.usage.output_tokens,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_clients::ThinkingLevel;

    #[test]
    fn parses_responses_output_array() {
        let json = r#"{
            "model": "gpt-4o-mini",
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {"type": "output_text", "text": "Hi there"}
                    ]
                }
            ],
            "usage": {"input_tokens": 10, "output_tokens": 3, "total_tokens": 13}
        }"#;
        let parsed: ResponsesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(extract_text(&parsed), "Hi there");
        assert_eq!(parsed.usage.input_tokens, 10);
        assert_eq!(parsed.usage.output_tokens, 3);
    }

    #[test]
    fn prefers_output_text_aggregate() {
        let json = r#"{
            "model": "gpt-4o-mini",
            "output_text": "aggregate wins",
            "output": [
                {"type": "message", "content": [{"type": "output_text", "text": "ignored"}]}
            ],
            "usage": {"input_tokens": 1, "output_tokens": 2}
        }"#;
        let parsed: ResponsesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(extract_text(&parsed), "aggregate wins");
    }

    #[test]
    fn build_request_maps_system_to_instructions() {
        let req = LlmRequest::new("gpt-x", "question").with_system("you are a bot");
        let body = build_request(&req);
        assert_eq!(body.input, "question");
        assert_eq!(body.instructions.as_deref(), Some("you are a bot"));
        assert!(body.reasoning.is_none());
    }

    #[test]
    fn build_request_omits_empty_instructions() {
        let body = build_request(&LlmRequest::new("gpt-x", "q"));
        assert!(body.instructions.is_none());
    }

    #[test]
    fn build_request_sets_reasoning_effort_when_thinking() {
        let req = LlmRequest::new("o4-mini", "q").with_thinking(ThinkingLevel::High);
        let body = build_request(&req);
        assert_eq!(body.reasoning.map(|r| r.effort).as_deref(), Some("high"));
    }
}
