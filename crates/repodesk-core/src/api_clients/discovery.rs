use serde::Deserialize;
use reqwest::Client;
use std::time::Duration;

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Deserialize)]
struct OllamaModel {
    name: String,
}

#[derive(Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Deserialize)]
struct OpenAiModel {
    id: String,
}

pub async fn fetch_ollama_models(base_url: &str) -> Result<Vec<String>, String> {
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    
    let client = Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Ollama at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Ollama API returned status {}", response.status()));
    }

    let parsed: OllamaTagsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {e}"))?;

    let mut models = parsed.models.into_iter().map(|m| m.name).collect::<Vec<_>>();
    models.sort();
    Ok(models)
}

pub async fn fetch_lm_studio_models(base_url: &str) -> Result<Vec<String>, String> {
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    
    let client = Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to LM Studio at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("LM Studio API returned status {}", response.status()));
    }

    let parsed: OpenAiModelsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse LM Studio response: {e}"))?;

    let mut models = parsed.data.into_iter().map(|m| m.id).collect::<Vec<_>>();
    models.sort();
    Ok(models)
}
