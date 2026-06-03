//! Google Gemini client implementing [`super::LlmProvider`] via the
//! `generateContent` endpoint. The [`super::ThinkingLevel`] hint is not mapped
//! here and is ignored.

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};

use super::{LlmRequest, LlmResponse, ProviderFuture};

const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com";
const DEFAULT_MODEL: &str = "gemini-2.0-flash";

pub struct GeminiClient {
    api_key: String,
    base_url: String,
    client: Client,
}

impl GeminiClient {
    pub fn new(api_key: String, base_url: Option<String>) -> Self {
        Self {
            api_key,
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            client: Client::new(),
        }
    }
}

#[derive(Serialize)]
struct GenerateRequest {
    contents: Vec<Content>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    system_instruction: Option<Content>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Serialize)]
struct GenerationConfig {
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
}

#[derive(Serialize, Deserialize)]
struct Content {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    parts: Vec<Part>,
}

#[derive(Serialize, Deserialize)]
struct Part {
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct GenerateResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
    #[serde(rename = "usageMetadata", default)]
    usage: UsageMetadata,
}

#[derive(Deserialize)]
struct Candidate {
    #[serde(default)]
    content: Option<Content>,
}

#[derive(Deserialize, Default)]
struct UsageMetadata {
    #[serde(rename = "promptTokenCount", default)]
    prompt_token_count: usize,
    #[serde(rename = "candidatesTokenCount", default)]
    candidates_token_count: usize,
}

fn extract_text(candidates: &[Candidate]) -> String {
    candidates
        .first()
        .and_then(|c| c.content.as_ref())
        .map(|content| {
            content
                .parts
                .iter()
                .map(|p| p.text.as_str())
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

impl super::LlmProvider for GeminiClient {
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
        let model = if request.model.trim().is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            request.model.clone()
        };
        let url = format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            self.base_url, model, self.api_key
        );
        let client = self.client.clone();
        let body = GenerateRequest {
            contents: vec![Content {
                role: Some("user".to_string()),
                parts: vec![Part {
                    text: request.prompt.clone(),
                }],
            }],
            system_instruction: request
                .system
                .as_ref()
                .filter(|s| !s.trim().is_empty())
                .map(|system| Content {
                    role: None,
                    parts: vec![Part {
                        text: system.clone(),
                    }],
                }),
            generation_config: GenerationConfig {
                max_output_tokens: request.max_tokens,
            },
        };

        Box::pin(async move {
            let resp = client.post(&url).json(&body).send().await.map_err(|e| {
                RepoDeskError::ProviderUnavailable {
                    provider: "gemini".to_string(),
                    detail: format!("request failed: {e}"),
                }
            })?;

            let status = resp.status();
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                return Err(RepoDeskError::ProviderRateLimit {
                    provider: "gemini".to_string(),
                    retry_after_secs: 60,
                });
            }
            if !status.is_success() {
                let detail = resp.text().await.unwrap_or_default();
                return Err(RepoDeskError::Api(format!(
                    "Gemini API error {status}: {detail}"
                )));
            }

            let parsed: GenerateResponse = resp
                .json()
                .await
                .map_err(|e| RepoDeskError::Api(format!("failed to parse Gemini response: {e}")))?;

            Ok(LlmResponse {
                text: extract_text(&parsed.candidates),
                model,
                input_tokens: parsed.usage.prompt_token_count,
                output_tokens: parsed.usage.candidates_token_count,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_generate_response() {
        let json = r#"{
            "candidates": [
                {"content": {"role": "model", "parts": [{"text": "Part A "}, {"text": "Part B"}]}}
            ],
            "usageMetadata": {"promptTokenCount": 7, "candidatesTokenCount": 4}
        }"#;
        let parsed: GenerateResponse = serde_json::from_str(json).unwrap();
        assert_eq!(extract_text(&parsed.candidates), "Part A Part B");
        assert_eq!(parsed.usage.prompt_token_count, 7);
        assert_eq!(parsed.usage.candidates_token_count, 4);
    }

    #[test]
    fn empty_candidates_yield_empty_text() {
        let parsed: GenerateResponse = serde_json::from_str(r#"{"candidates": []}"#).unwrap();
        assert_eq!(extract_text(&parsed.candidates), "");
    }
}
