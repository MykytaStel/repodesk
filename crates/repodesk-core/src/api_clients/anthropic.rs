//! Anthropic (Claude) client implementing [`super::LlmProvider`] via the
//! Messages API. Maps [`super::ThinkingLevel`] onto Anthropic's extended-thinking
//! `budget_tokens`, and returns real `usage` token counts for the ledger.

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};

use super::{LlmRequest, LlmResponse, ProviderFuture};

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Default model for the back-compat `generate` path; orchestrated calls always
/// pass an explicit model on the request.
const DEFAULT_MODEL: &str = "claude-sonnet-4-6";

pub struct AnthropicClient {
    api_key: String,
    base_url: String,
    client: Client,
}

impl AnthropicClient {
    pub fn new(api_key: String, base_url: Option<String>) -> Self {
        Self {
            api_key,
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            client: Client::new(),
        }
    }
}

#[derive(Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ThinkingConfig>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ThinkingConfig {
    #[serde(rename = "type")]
    kind: String,
    budget_tokens: u32,
}

#[derive(Deserialize)]
struct MessagesResponse {
    #[serde(default)]
    content: Vec<ContentBlock>,
    #[serde(default)]
    model: String,
    #[serde(default)]
    usage: Usage,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type", default)]
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

/// Concatenate the `text` content blocks, ignoring `thinking`/other block types.
fn extract_text(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter(|b| b.kind == "text")
        .map(|b| b.text.as_str())
        .collect::<Vec<_>>()
        .join("")
}

fn build_request(request: &LlmRequest) -> MessagesRequest {
    let model = if request.model.trim().is_empty() {
        DEFAULT_MODEL.to_string()
    } else {
        request.model.clone()
    };
    let thinking = request
        .thinking
        .thinking_budget_tokens()
        .map(|budget_tokens| ThinkingConfig {
            kind: "enabled".to_string(),
            budget_tokens,
        });
    // Anthropic requires max_tokens strictly greater than the thinking budget.
    let max_tokens = match request.thinking.thinking_budget_tokens() {
        Some(budget) => request.max_tokens.max(budget.saturating_add(1_024)),
        None => request.max_tokens,
    };
    MessagesRequest {
        model,
        max_tokens,
        messages: vec![Message {
            role: "user".to_string(),
            content: request.prompt.clone(),
        }],
        system: request.system.clone().filter(|s| !s.trim().is_empty()),
        thinking,
    }
}

impl super::LlmProvider for AnthropicClient {
    fn generate(
        &self,
        prompt: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = RepoDeskResult<String>> + Send>> {
        let fut = self.complete(LlmRequest::new(DEFAULT_MODEL, prompt));
        Box::pin(async move { fut.await.map(|r| r.text) })
    }

    fn is_available(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>> {
        // No free ping endpoint; treat a present key as "available". Real errors
        // surface on the first `complete` call.
        let ok = !self.api_key.trim().is_empty();
        Box::pin(async move { ok })
    }

    fn complete(&self, request: LlmRequest) -> ProviderFuture<RepoDeskResult<LlmResponse>> {
        let url = format!("{}/v1/messages", self.base_url);
        let api_key = self.api_key.clone();
        let client = self.client.clone();
        let body = build_request(&request);
        let model = body.model.clone();

        Box::pin(async move {
            let resp = client
                .post(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .json(&body)
                .send()
                .await
                .map_err(|e| RepoDeskError::ProviderUnavailable {
                    provider: "anthropic".to_string(),
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
                    provider: "anthropic".to_string(),
                    retry_after_secs,
                });
            }
            if !status.is_success() {
                let detail = resp.text().await.unwrap_or_default();
                return Err(RepoDeskError::Api(format!(
                    "Anthropic API error {status}: {detail}"
                )));
            }

            let parsed: MessagesResponse = resp.json().await.map_err(|e| {
                RepoDeskError::Api(format!("failed to parse Anthropic response: {e}"))
            })?;

            Ok(LlmResponse {
                text: extract_text(&parsed.content),
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
    fn parses_messages_response_and_joins_text_blocks() {
        let json = r#"{
            "id": "msg_1",
            "model": "claude-sonnet-4-6",
            "content": [
                {"type": "thinking", "thinking": "hmm"},
                {"type": "text", "text": "Hello "},
                {"type": "text", "text": "world"}
            ],
            "usage": {"input_tokens": 12, "output_tokens": 5}
        }"#;
        let parsed: MessagesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(extract_text(&parsed.content), "Hello world");
        assert_eq!(parsed.usage.input_tokens, 12);
        assert_eq!(parsed.usage.output_tokens, 5);
        assert_eq!(parsed.model, "claude-sonnet-4-6");
    }

    #[test]
    fn thinking_request_bumps_max_tokens_above_budget() {
        let req = LlmRequest::new("claude-x", "do work")
            .with_max_tokens(256)
            .with_thinking(ThinkingLevel::High);
        let body = build_request(&req);
        let budget = ThinkingLevel::High.thinking_budget_tokens().unwrap();
        assert!(body.thinking.is_some());
        assert!(
            body.max_tokens > budget,
            "max_tokens must exceed thinking budget"
        );
    }

    #[test]
    fn no_thinking_leaves_request_lean() {
        let req = LlmRequest::new("claude-x", "hi").with_system("be terse");
        let body = build_request(&req);
        assert!(body.thinking.is_none());
        assert_eq!(body.system.as_deref(), Some("be terse"));
        assert_eq!(body.max_tokens, 1_024);
    }
}
