use crate::errors::{RepoDeskError, RepoDeskResult};
use serde::{Deserialize, Serialize};

/// Interface for generating text embeddings.
pub trait EmbeddingProvider: Send + Sync {
    /// Generate an embedding vector for the given text.
    fn get_embedding(&self, text: &str) -> RepoDeskResult<Vec<f32>>;
}

#[derive(Clone)]
pub struct OllamaEmbeddingProvider {
    pub api_base: String,
    pub model: String,
}

#[derive(Serialize)]
struct OllamaEmbeddingRequest<'a> {
    model: &'a str,
    prompt: &'a str,
}

#[derive(Deserialize)]
struct OllamaEmbeddingResponse {
    embedding: Vec<f32>,
}

impl EmbeddingProvider for OllamaEmbeddingProvider {
    fn get_embedding(&self, text: &str) -> RepoDeskResult<Vec<f32>> {
        let endpoint = format!("{}/api/embeddings", self.api_base.trim_end_matches('/'));
        let request = OllamaEmbeddingRequest {
            model: &self.model,
            prompt: text,
        };

        let response = ureq::post(&endpoint)
            .send_json(request)
            .map_err(|e| RepoDeskError::Api(format!("Ollama embedding request failed: {e}")))?;

        let resp: OllamaEmbeddingResponse = response.into_body().read_json().map_err(|e| {
            RepoDeskError::Api(format!("Failed to parse Ollama embedding response: {e}"))
        })?;

        Ok(resp.embedding)
    }
}
