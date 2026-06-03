use crate::errors::{RepoDeskError, RepoDeskResult};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaTag {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaTagsResponse {
    pub models: Vec<OllamaTag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaGenerateRequest {
    pub model: String,
    pub prompt: String,
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaGenerateResponse {
    pub response: String,
}

pub struct OllamaClient {
    base_url: String,
    client: Client,
    default_model: String,
}

impl OllamaClient {
    pub fn new(base_url: Option<String>, default_model: Option<String>) -> Self {
        Self {
            base_url: base_url.unwrap_or_else(|| "http://localhost:11434".to_string()),
            client: Client::new(),
            default_model: default_model.unwrap_or_else(|| "llama3.1".to_string()),
        }
    }

    pub async fn get_tags(&self) -> RepoDeskResult<Vec<String>> {
        let url = format!("{}/api/tags", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| RepoDeskError::Api(format!("Failed to connect to Ollama: {}", e)))?;

        if !resp.status().is_success() {
            return Err(RepoDeskError::Api(format!(
                "Ollama API error: {}",
                resp.status()
            )));
        }

        let tags: OllamaTagsResponse = resp
            .json()
            .await
            .map_err(|e| RepoDeskError::Api(format!("Failed to parse Ollama tags: {}", e)))?;

        Ok(tags.models.into_iter().map(|m| m.name).collect())
    }
}

impl super::LlmProvider for OllamaClient {
    fn generate(
        &self,
        prompt: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = RepoDeskResult<String>> + Send>> {
        let url = format!("{}/api/generate", self.base_url);
        let req = OllamaGenerateRequest {
            model: self.default_model.clone(),
            prompt: prompt.to_string(),
            stream: false,
        };
        let client = self.client.clone();

        Box::pin(async move {
            let resp =
                client.post(&url).json(&req).send().await.map_err(|e| {
                    RepoDeskError::Api(format!("Failed to connect to Ollama: {}", e))
                })?;

            if !resp.status().is_success() {
                return Err(RepoDeskError::Api(format!(
                    "Ollama API error: {}",
                    resp.status()
                )));
            }

            let gen_resp: OllamaGenerateResponse = resp.json().await.map_err(|e| {
                RepoDeskError::Api(format!("Failed to parse Ollama response: {}", e))
            })?;

            Ok(gen_resp.response)
        })
    }

    fn is_available(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>> {
        let url = format!("{}/api/tags", self.base_url);
        let client = self.client.clone();

        Box::pin(async move { client.get(&url).send().await.is_ok() })
    }

    fn complete(
        &self,
        request: super::LlmRequest,
    ) -> super::ProviderFuture<RepoDeskResult<super::LlmResponse>> {
        let url = format!("{}/api/generate", self.base_url);
        // Ollama has no system field on /api/generate; prepend it to the prompt.
        let prompt = match &request.system {
            Some(system) if !system.trim().is_empty() => {
                format!("{system}\n\n{}", request.prompt)
            }
            _ => request.prompt.clone(),
        };
        // The request model wins; fall back to the client default.
        let model = if request.model.trim().is_empty() {
            self.default_model.clone()
        } else {
            request.model.clone()
        };
        let req = OllamaGenerateRequest {
            model: model.clone(),
            prompt: prompt.clone(),
            stream: false,
        };
        let client = self.client.clone();

        Box::pin(async move {
            let resp = client.post(&url).json(&req).send().await.map_err(|e| {
                RepoDeskError::ProviderUnavailable {
                    provider: "ollama".to_string(),
                    detail: format!("failed to connect to Ollama: {e}"),
                }
            })?;

            if !resp.status().is_success() {
                return Err(RepoDeskError::Api(format!(
                    "Ollama API error: {}",
                    resp.status()
                )));
            }

            let gen_resp: OllamaGenerateResponse = resp
                .json()
                .await
                .map_err(|e| RepoDeskError::Api(format!("Failed to parse Ollama response: {e}")))?;

            // Ollama's basic response carries no usage counts; estimate them so
            // the call still shows up in the token ledger.
            let input_tokens = crate::tokens::estimate_text(&prompt).estimated_tokens;
            let output_tokens = crate::tokens::estimate_text(&gen_resp.response).estimated_tokens;

            Ok(super::LlmResponse {
                text: gen_resp.response,
                model,
                input_tokens,
                output_tokens,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ollama_tags() {
        let json = r#"{
            "models": [
                {"name": "llama3.1:latest", "modified_at": "2024-05-29T10:00:00Z", "size": 4700000000},
                {"name": "deepseek-coder:latest", "modified_at": "2024-05-29T10:00:00Z", "size": 3900000000}
            ]
        }"#;

        let response: OllamaTagsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.models.len(), 2);
        assert_eq!(response.models[0].name, "llama3.1:latest");
        assert_eq!(response.models[1].name, "deepseek-coder:latest");
    }

    #[test]
    fn parses_ollama_generate_response() {
        let json = r#"{
            "model": "llama3.1:latest",
            "created_at": "2024-05-29T10:00:00Z",
            "response": "Hello world",
            "done": true
        }"#;

        let response: OllamaGenerateResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.response, "Hello world");
    }
}
