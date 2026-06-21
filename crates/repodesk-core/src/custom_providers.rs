//! User-added **OpenAI-compatible** providers (DeepSeek, Groq, OpenRouter,
//! Together, Mistral, xAI, self-hosted, …). RepoDesk shouldn't be limited to the
//! handful of built-in vendors: any service that speaks the OpenAI Chat
//! Completions API can be added here with a base URL + key + default model, and
//! is then routable like any other completion provider (via
//! [`crate::api_clients::OpenAiCompatClient`]).
//!
//! Stored in `custom_providers.toml` (the [`ConfigStore`] pattern, local file
//! under the RepoDesk config dir). Presets fill the base URL so the user only
//! pastes a key.

use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::utils::ConfigStore;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomProvider {
    /// Canonical, unique id used for routing and overrides (e.g. `deepseek`).
    pub id: String,
    pub label: String,
    /// API root, e.g. `https://api.deepseek.com` (no `/v1` suffix).
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub default_model: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl CustomProvider {
    pub fn has_api_key(&self) -> bool {
        !self.api_key.trim().is_empty()
    }

    pub fn requires_api_key(&self) -> bool {
        !is_local_base_url(&self.base_url)
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct CustomProvidersConfig {
    #[serde(default)]
    pub providers: Vec<CustomProvider>,
}

impl ConfigStore for CustomProvidersConfig {
    const FILE_NAME: &'static str = "custom_providers.toml";
}

/// A ready-made base URL for a known OpenAI-compatible vendor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPreset {
    pub id: String,
    pub label: String,
    pub base_url: String,
    pub default_model: String,
    /// Env var the vendor's key conventionally lives in (hint for the UI).
    pub key_env_hint: String,
}

/// Curated OpenAI-compatible vendors with their base URLs filled in.
pub fn presets() -> Vec<ProviderPreset> {
    let p = |id: &str, label: &str, base_url: &str, model: &str, env: &str| ProviderPreset {
        id: id.to_string(),
        label: label.to_string(),
        base_url: base_url.to_string(),
        default_model: model.to_string(),
        key_env_hint: env.to_string(),
    };
    vec![
        p(
            "deepseek",
            "DeepSeek",
            "https://api.deepseek.com",
            "deepseek-chat",
            "DEEPSEEK_API_KEY",
        ),
        p(
            "openrouter",
            "OpenRouter",
            "https://openrouter.ai/api",
            "openai/gpt-4o-mini",
            "OPENROUTER_API_KEY",
        ),
        p(
            "groq",
            "Groq",
            "https://api.groq.com/openai",
            "llama-3.3-70b-versatile",
            "GROQ_API_KEY",
        ),
        p(
            "together",
            "Together AI",
            "https://api.together.xyz",
            "meta-llama/Llama-3.3-70B-Instruct-Turbo",
            "TOGETHER_API_KEY",
        ),
        p(
            "mistral",
            "Mistral",
            "https://api.mistral.ai",
            "mistral-large-latest",
            "MISTRAL_API_KEY",
        ),
        p(
            "xai",
            "xAI (Grok)",
            "https://api.x.ai",
            "grok-2-latest",
            "XAI_API_KEY",
        ),
    ]
}

pub fn is_local_base_url(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized.starts_with("http://localhost")
        || normalized.starts_with("http://127.0.0.1")
        || normalized.starts_with("http://[::1]")
}

fn slugify(value: &str) -> String {
    let mapped: String = value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let collapsed = mapped
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if collapsed.is_empty() {
        "provider".to_string()
    } else {
        collapsed
    }
}

/// All configured custom providers.
pub fn list_custom_providers() -> RepoDeskResult<Vec<CustomProvider>> {
    Ok(CustomProvidersConfig::load_config()?.providers)
}

/// Create or update a custom provider (matched by id; blank id derived from
/// label). Validates the base URL is http(s). Returns the full list.
pub fn save_custom_provider(mut provider: CustomProvider) -> RepoDeskResult<Vec<CustomProvider>> {
    if provider.label.trim().is_empty() {
        return Err(RepoDeskError::RoutingFailed {
            detail: "a custom provider needs a label".to_string(),
        });
    }
    let base = provider.base_url.trim();
    if !(base.starts_with("http://") || base.starts_with("https://")) {
        return Err(RepoDeskError::RoutingFailed {
            detail: "base URL must start with http:// or https://".to_string(),
        });
    }
    provider.base_url = base.trim_end_matches('/').to_string();
    if provider.id.trim().is_empty() {
        provider.id = slugify(&provider.label);
    } else {
        provider.id = slugify(&provider.id);
    }

    let mut config = CustomProvidersConfig::load_config()?;
    if let Some(existing) = config.providers.iter_mut().find(|p| p.id == provider.id) {
        *existing = provider;
    } else {
        config.providers.push(provider);
    }
    config.save_config()?;
    Ok(config.providers)
}

/// Delete a custom provider by id. Returns the full list.
pub fn delete_custom_provider(id: &str) -> RepoDeskResult<Vec<CustomProvider>> {
    let mut config = CustomProvidersConfig::load_config()?;
    let before = config.providers.len();
    config.providers.retain(|p| p.id != id);
    if config.providers.len() == before {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!("no custom provider with id '{id}'"),
        });
    }
    config.save_config()?;
    Ok(config.providers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_have_https_base_urls() {
        for preset in presets() {
            assert!(preset.base_url.starts_with("https://"), "{}", preset.id);
            assert!(!preset.default_model.is_empty());
        }
    }

    #[test]
    fn slugify_normalizes_ids() {
        assert_eq!(slugify("My Local LLM!"), "my-local-llm");
        assert_eq!(slugify(""), "provider");
    }

    #[test]
    fn local_base_url_detection_is_loopback_only() {
        assert!(is_local_base_url("http://localhost:1234"));
        assert!(is_local_base_url("http://127.0.0.1:1234"));
        assert!(!is_local_base_url("https://api.deepseek.com"));
    }
}
