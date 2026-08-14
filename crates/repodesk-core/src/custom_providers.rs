//! User-added **OpenAI-compatible** providers (DeepSeek, Groq, OpenRouter,
//! Together, Mistral, xAI, self-hosted, …). RepoDesk shouldn't be limited to the
//! handful of built-in vendors: any service that speaks the OpenAI Chat
//! Completions API can be added here with a base URL + key + default model, and
//! is then routable like any other completion provider (via
//! [`crate::api_clients::OpenAiCompatClient`]).
//!
//! Provider metadata lives in `custom_providers.toml`; API keys live only in the
//! OS keychain through [`crate::credentials::CredentialResolver`]. The
//! `api_key` field below is deliberately **deserialize-only** so legacy TOML can
//! be migrated and runtime callers can carry a resolved key without any config
//! serialization path being able to write the secret back to disk.

use serde::{Deserialize, Serialize};

use crate::credentials::{CredentialResolver, default_resolver};
use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::utils::ConfigStore;

const CUSTOM_PROVIDER_KEY_PREFIX: &str = "custom_provider";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomProvider {
    /// Canonical, unique id used for routing and overrides (e.g. `deepseek`).
    pub id: String,
    pub label: String,
    /// API root, e.g. `https://api.deepseek.com` (no `/v1` suffix).
    pub base_url: String,
    /// Transient input/runtime credential. This field is intentionally never
    /// serialized, which makes plaintext persistence impossible through
    /// `CustomProvidersConfig::save_config`. It is still deserialized so an old
    /// `api_key = "..."` entry can be migrated once into the OS keychain.
    #[serde(default, skip_serializing)]
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

fn base_url_host(value: &str) -> Option<String> {
    let normalized = value.trim();
    let rest = normalized
        .strip_prefix("http://")
        .or_else(|| normalized.strip_prefix("https://"))?;
    let authority = rest.split(['/', '?', '#']).next()?.trim();
    if authority.is_empty() || authority.contains('@') {
        return None;
    }

    let host = if let Some(bracketed) = authority.strip_prefix('[') {
        let end = bracketed.find(']')?;
        &bracketed[..end]
    } else {
        authority.split(':').next().unwrap_or(authority)
    };

    if host.is_empty() {
        None
    } else {
        Some(host.to_ascii_lowercase())
    }
}

pub fn is_local_base_url(value: &str) -> bool {
    matches!(
        base_url_host(value).as_deref(),
        Some("localhost" | "127.0.0.1" | "::1")
    )
}

fn validate_provider_base_url(value: &str) -> RepoDeskResult<String> {
    let base = value.trim().trim_end_matches('/');
    if base_url_host(base).is_none() {
        return Err(RepoDeskError::RoutingFailed {
            detail: "base URL must be a valid http:// or https:// URL without embedded credentials"
                .to_string(),
        });
    }

    if base.starts_with("https://") {
        return Ok(base.to_string());
    }

    if base.starts_with("http://") && is_local_base_url(base) {
        return Ok(base.to_string());
    }

    Err(RepoDeskError::RoutingFailed {
        detail: "remote custom providers must use HTTPS; plain HTTP is allowed only for localhost/127.0.0.1/::1"
            .to_string(),
    })
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

/// Deterministic keychain account name for one normalized custom provider id.
/// The provider id is metadata, not a secret, and is already restricted to
/// lower-case ASCII alphanumeric characters plus `-` by [`slugify`].
pub fn credential_key_for_provider(id: &str) -> String {
    format!("{CUSTOM_PROVIDER_KEY_PREFIX}_{}_api_key", slugify(id))
}

fn verify_stored_secret(
    resolver: &dyn CredentialResolver,
    key: &str,
    expected: &str,
) -> RepoDeskResult<()> {
    let stored = resolver.get(key)?;
    if stored.as_deref() != Some(expected) {
        return Err(RepoDeskError::Api(
            "custom provider keychain write could not be verified".to_string(),
        ));
    }
    Ok(())
}

/// Move any legacy plaintext `api_key` values into the supplied credential
/// resolver. Secrets are cleared from the in-memory config only *after* the
/// keychain write has been read back successfully. The caller persists the
/// sanitized config only when this returns `Ok(true)`.
fn migrate_plaintext_credentials(
    config: &mut CustomProvidersConfig,
    resolver: &dyn CredentialResolver,
) -> RepoDeskResult<bool> {
    let mut changed = false;
    for provider in &mut config.providers {
        let secret = provider.api_key.trim().to_string();
        if secret.is_empty() {
            continue;
        }
        let key = credential_key_for_provider(&provider.id);
        resolver.set(&key, &secret)?;
        verify_stored_secret(resolver, &key, &secret)?;
        provider.api_key.clear();
        changed = true;
    }
    Ok(changed)
}

fn load_config_with_migration(
    resolver: &dyn CredentialResolver,
) -> RepoDeskResult<CustomProvidersConfig> {
    let mut config = CustomProvidersConfig::load_config()?;
    if migrate_plaintext_credentials(&mut config, resolver)? {
        // `CustomProvider::api_key` is skip_serializing, so this rewrite removes
        // every legacy plaintext key from disk. If persistence fails, propagate
        // the error and do not continue with a partially trusted provider list.
        config.save_config()?;
    }
    Ok(config)
}

fn hydrate_credentials(
    mut providers: Vec<CustomProvider>,
    resolver: &dyn CredentialResolver,
) -> RepoDeskResult<Vec<CustomProvider>> {
    for provider in &mut providers {
        let key = credential_key_for_provider(&provider.id);
        provider.api_key = resolver.get(&key)?.unwrap_or_default();
    }
    Ok(providers)
}

fn restore_previous_secret(
    resolver: &dyn CredentialResolver,
    key: &str,
    previous: Option<&str>,
) -> RepoDeskResult<()> {
    match previous {
        Some(value) => resolver.set(key, value),
        None => resolver.delete(key),
    }
}

fn save_custom_provider_with(
    mut provider: CustomProvider,
    resolver: &dyn CredentialResolver,
) -> RepoDeskResult<Vec<CustomProvider>> {
    if provider.label.trim().is_empty() {
        return Err(RepoDeskError::RoutingFailed {
            detail: "a custom provider needs a label".to_string(),
        });
    }
    provider.base_url = validate_provider_base_url(&provider.base_url)?;
    if provider.id.trim().is_empty() {
        provider.id = slugify(&provider.label);
    } else {
        provider.id = slugify(&provider.id);
    }

    let mut config = load_config_with_migration(resolver)?;
    let key = credential_key_for_provider(&provider.id);
    let previous_secret = resolver.get(&key)?;
    let new_secret = provider.api_key.trim().to_string();

    if new_secret.is_empty() {
        resolver.delete(&key)?;
        if resolver.get(&key)?.is_some() {
            return Err(RepoDeskError::Api(
                "custom provider keychain delete could not be verified".to_string(),
            ));
        }
    } else {
        resolver.set(&key, &new_secret)?;
        verify_stored_secret(resolver, &key, &new_secret)?;
    }

    // Never keep a credential in the metadata object we are about to persist.
    // The serde guard also omits this field, but clearing it makes the boundary
    // explicit and keeps the in-memory config non-secret too.
    provider.api_key.clear();
    if let Some(existing) = config.providers.iter_mut().find(|p| p.id == provider.id) {
        *existing = provider;
    } else {
        config.providers.push(provider);
    }

    if let Err(save_error) = config.save_config() {
        // Metadata did not commit. Restore the previous credential state so the
        // keychain and provider config do not silently diverge.
        if restore_previous_secret(resolver, &key, previous_secret.as_deref()).is_err() {
            return Err(RepoDeskError::Api(
                "custom provider metadata save failed and keychain rollback also failed"
                    .to_string(),
            ));
        }
        return Err(save_error);
    }

    hydrate_credentials(config.providers, resolver)
}

fn delete_custom_provider_with(
    id: &str,
    resolver: &dyn CredentialResolver,
) -> RepoDeskResult<Vec<CustomProvider>> {
    let mut config = load_config_with_migration(resolver)?;
    let normalized = slugify(id);
    let before = config.providers.len();
    config.providers.retain(|p| p.id != normalized);
    if config.providers.len() == before {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!("no custom provider with id '{id}'"),
        });
    }

    let key = credential_key_for_provider(&normalized);
    let previous_secret = resolver.get(&key)?;
    resolver.delete(&key)?;
    if resolver.get(&key)?.is_some() {
        return Err(RepoDeskError::Api(
            "custom provider keychain delete could not be verified".to_string(),
        ));
    }

    if let Err(save_error) = config.save_config() {
        if restore_previous_secret(resolver, &key, previous_secret.as_deref()).is_err() {
            return Err(RepoDeskError::Api(
                "custom provider delete failed and keychain rollback also failed".to_string(),
            ));
        }
        return Err(save_error);
    }

    hydrate_credentials(config.providers, resolver)
}

/// All configured custom providers. Credentials are resolved from the OS
/// keychain at call time; `custom_providers.toml` contains metadata only.
pub fn list_custom_providers() -> RepoDeskResult<Vec<CustomProvider>> {
    let resolver = default_resolver();
    let config = load_config_with_migration(resolver.as_ref())?;
    hydrate_credentials(config.providers, resolver.as_ref())
}

/// Create or update a custom provider (matched by id; blank id derived from
/// label). Metadata is written to TOML and the key is written to the OS keychain.
pub fn save_custom_provider(provider: CustomProvider) -> RepoDeskResult<Vec<CustomProvider>> {
    let resolver = default_resolver();
    save_custom_provider_with(provider, resolver.as_ref())
}

/// Delete a custom provider and its keychain credential. Returns the full list.
pub fn delete_custom_provider(id: &str) -> RepoDeskResult<Vec<CustomProvider>> {
    let resolver = default_resolver();
    delete_custom_provider_with(id, resolver.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemoryResolver(Mutex<HashMap<String, String>>);

    impl CredentialResolver for MemoryResolver {
        fn get(&self, key: &str) -> RepoDeskResult<Option<String>> {
            Ok(self.0.lock().unwrap().get(key).cloned())
        }

        fn set(&self, key: &str, value: &str) -> RepoDeskResult<()> {
            self.0
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
            Ok(())
        }

        fn delete(&self, key: &str) -> RepoDeskResult<()> {
            self.0.lock().unwrap().remove(key);
            Ok(())
        }
    }

    fn provider_with_key(key: &str) -> CustomProvider {
        CustomProvider {
            id: "Deep Seek".to_string(),
            label: "DeepSeek".to_string(),
            base_url: "https://api.deepseek.com".to_string(),
            api_key: key.to_string(),
            default_model: "deepseek-chat".to_string(),
            enabled: true,
        }
    }

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
    fn local_base_url_detection_is_exactly_loopback() {
        assert!(is_local_base_url("http://localhost:1234"));
        assert!(is_local_base_url("http://127.0.0.1:1234"));
        assert!(is_local_base_url("http://[::1]:1234"));
        assert!(is_local_base_url("https://localhost:1234"));
        assert!(!is_local_base_url("http://localhost.evil.example"));
        assert!(!is_local_base_url("http://127.0.0.1.evil.example"));
        assert!(!is_local_base_url("https://api.deepseek.com"));
    }

    #[test]
    fn remote_provider_transport_requires_https() {
        assert!(validate_provider_base_url("https://api.example.com").is_ok());
        assert!(validate_provider_base_url("http://localhost:11434").is_ok());
        assert!(validate_provider_base_url("http://127.0.0.1:1234").is_ok());
        assert!(validate_provider_base_url("http://api.example.com").is_err());
        assert!(validate_provider_base_url("http://localhost.evil.example").is_err());
        assert!(validate_provider_base_url("https://user:pass@example.com").is_err());
    }

    #[test]
    fn rejected_remote_http_provider_never_writes_a_credential() {
        let resolver = MemoryResolver::default();
        let mut provider = provider_with_key("fixture-secret-that-must-stay-local");
        provider.base_url = "http://api.example.com".to_string();

        assert!(save_custom_provider_with(provider, &resolver).is_err());
        assert!(resolver.0.lock().unwrap().is_empty());
    }

    #[test]
    fn custom_provider_serialization_never_contains_api_key() {
        let provider = provider_with_key("fixture-plaintext-must-not-persist");
        let serialized = toml::to_string_pretty(&CustomProvidersConfig {
            providers: vec![provider],
        })
        .unwrap();
        assert!(!serialized.contains("api_key"));
        assert!(!serialized.contains("fixture-plaintext-must-not-persist"));
    }

    #[test]
    fn legacy_plaintext_key_migrates_and_is_cleared_only_after_verification() {
        let resolver = MemoryResolver::default();
        let mut config = CustomProvidersConfig {
            providers: vec![provider_with_key("fixture-legacy-secret")],
        };

        assert!(migrate_plaintext_credentials(&mut config, &resolver).unwrap());
        assert!(config.providers[0].api_key.is_empty());
        let credential_key = credential_key_for_provider("Deep Seek");
        assert_eq!(
            resolver.get(&credential_key).unwrap().as_deref(),
            Some("fixture-legacy-secret")
        );

        let sanitized = toml::to_string_pretty(&config).unwrap();
        assert!(!sanitized.contains("fixture-legacy-secret"));
        assert!(!sanitized.contains("api_key"));
    }

    #[test]
    fn legacy_toml_still_deserializes_for_one_time_migration() {
        let config: CustomProvidersConfig = toml::from_str(
            r#"
                [[providers]]
                id = "deepseek"
                label = "DeepSeek"
                base_url = "https://api.deepseek.com"
                api_key = "fixture"
                default_model = "deepseek-chat"
                enabled = true
            "#,
        )
        .unwrap();
        assert_eq!(config.providers[0].api_key, "fixture");
    }

    #[test]
    fn credential_key_is_stable_and_contains_no_secret() {
        assert_eq!(
            credential_key_for_provider("My Provider"),
            "custom_provider_my-provider_api_key"
        );
    }
}
