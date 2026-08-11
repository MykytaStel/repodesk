//! Desktop bridge for user-added OpenAI-compatible providers (DeepSeek, Groq,
//! OpenRouter, …): list, save (create/update), delete, and presets.
//!
//! Provider metadata is persisted by core in `custom_providers.toml`, while API
//! keys live in the OS keychain. Core's `CustomProvider::api_key` is deliberately
//! non-serializing, so this bridge uses a dedicated UI payload and only ever
//! sends a fixed mask outward — never a resolved secret.

use repodesk_core::custom_providers::{self, CustomProvider, ProviderPreset};
use serde::{Deserialize, Serialize};

use super::ErrorPayload;

const KEY_MASK: &str = "••••••••";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderPayload {
    pub id: String,
    pub label: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub default_model: String,
    pub enabled: bool,
}

impl From<CustomProvider> for CustomProviderPayload {
    fn from(provider: CustomProvider) -> Self {
        Self {
            id: provider.id,
            label: provider.label,
            base_url: provider.base_url,
            api_key: if provider.api_key.trim().is_empty() {
                String::new()
            } else {
                KEY_MASK.to_string()
            },
            default_model: provider.default_model,
            enabled: provider.enabled,
        }
    }
}

impl From<CustomProviderPayload> for CustomProvider {
    fn from(provider: CustomProviderPayload) -> Self {
        Self {
            id: provider.id,
            label: provider.label,
            base_url: provider.base_url,
            api_key: provider.api_key,
            default_model: provider.default_model,
            enabled: provider.enabled,
        }
    }
}

fn payloads(providers: Vec<CustomProvider>) -> Vec<CustomProviderPayload> {
    providers.into_iter().map(Into::into).collect()
}

/// All configured custom providers. Only key presence is exposed via a fixed
/// mask; resolved credentials never cross the Tauri boundary.
#[tauri::command]
pub async fn custom_providers_list() -> Result<Vec<CustomProviderPayload>, ErrorPayload> {
    Ok(payloads(custom_providers::list_custom_providers()?))
}

/// Curated OpenAI-compatible presets (base URLs filled in).
#[tauri::command]
pub async fn custom_providers_presets() -> Result<Vec<ProviderPreset>, ErrorPayload> {
    Ok(custom_providers::presets())
}

/// Create or update a custom provider; returns the full list with key presence
/// masked. A masked incoming key means "keep the existing key".
#[tauri::command]
pub async fn custom_providers_save(
    provider: CustomProviderPayload,
) -> Result<Vec<CustomProviderPayload>, ErrorPayload> {
    let mut provider: CustomProvider = provider.into();
    if provider.api_key == KEY_MASK {
        // Core resolves the existing credential from the OS keychain. Keep that
        // value entirely inside the backend and hand it straight back to core's
        // save path; it is never serialized to the WebView.
        provider.api_key = custom_providers::list_custom_providers()?
            .into_iter()
            .find(|p| p.id.eq_ignore_ascii_case(&provider.id))
            .map(|p| p.api_key)
            .unwrap_or_default();
    }
    Ok(payloads(custom_providers::save_custom_provider(provider)?))
}

/// Delete a custom provider and its keychain credential.
#[tauri::command]
pub async fn custom_providers_delete(
    id: String,
) -> Result<Vec<CustomProviderPayload>, ErrorPayload> {
    Ok(payloads(custom_providers::delete_custom_provider(&id)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_payload_masks_resolved_key() {
        let payload = CustomProviderPayload::from(CustomProvider {
            id: "deepseek".to_string(),
            label: "DeepSeek".to_string(),
            base_url: "https://api.deepseek.com".to_string(),
            api_key: "sk-secret-value".to_string(),
            default_model: "deepseek-chat".to_string(),
            enabled: true,
        });
        assert_eq!(payload.api_key, KEY_MASK);
        let json = serde_json::to_string(&payload).unwrap();
        assert!(!json.contains("sk-secret-value"));
    }

    #[test]
    fn ui_payload_keeps_empty_key_empty() {
        let payload = CustomProviderPayload::from(CustomProvider {
            id: "local".to_string(),
            label: "Local".to_string(),
            base_url: "http://127.0.0.1:8000".to_string(),
            api_key: String::new(),
            default_model: String::new(),
            enabled: true,
        });
        assert!(payload.api_key.is_empty());
    }
}
