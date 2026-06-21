//! Desktop bridge for user-added OpenAI-compatible providers (DeepSeek, Groq,
//! OpenRouter, …): list, save (create/update), delete, and presets. Backed by
//! `custom_providers.toml` in core.
//!
//! API keys are masked on the way out (like the built-in providers) and a saved
//! mask means "keep the existing key", so a key is never round-tripped to the UI
//! in clear text.

use repodesk_core::custom_providers::{self, CustomProvider, ProviderPreset};

use super::ErrorPayload;

const KEY_MASK: &str = "••••••••";

fn mask(mut provider: CustomProvider) -> CustomProvider {
    if !provider.api_key.trim().is_empty() {
        provider.api_key = KEY_MASK.to_string();
    }
    provider
}

fn mask_all(providers: Vec<CustomProvider>) -> Vec<CustomProvider> {
    providers.into_iter().map(mask).collect()
}

/// All configured custom providers (keys masked).
#[tauri::command]
pub async fn custom_providers_list() -> Result<Vec<CustomProvider>, ErrorPayload> {
    Ok(mask_all(custom_providers::list_custom_providers()?))
}

/// Curated OpenAI-compatible presets (base URLs filled in).
#[tauri::command]
pub async fn custom_providers_presets() -> Result<Vec<ProviderPreset>, ErrorPayload> {
    Ok(custom_providers::presets())
}

/// Create or update a custom provider; returns the full list (keys masked). A
/// masked incoming key means "keep the stored key".
#[tauri::command]
pub async fn custom_providers_save(
    mut provider: CustomProvider,
) -> Result<Vec<CustomProvider>, ErrorPayload> {
    if provider.api_key == KEY_MASK {
        let existing = custom_providers::list_custom_providers()?
            .into_iter()
            .find(|p| p.id.eq_ignore_ascii_case(&provider.id))
            .map(|p| p.api_key)
            .unwrap_or_default();
        provider.api_key = existing;
    }
    Ok(mask_all(custom_providers::save_custom_provider(provider)?))
}

/// Delete a custom provider by id; returns the full list (keys masked).
#[tauri::command]
pub async fn custom_providers_delete(id: String) -> Result<Vec<CustomProvider>, ErrorPayload> {
    Ok(mask_all(custom_providers::delete_custom_provider(&id)?))
}
