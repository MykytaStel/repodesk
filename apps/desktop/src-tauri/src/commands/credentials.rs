//! Desktop bridge for provider credentials. The frontend can store/delete the
//! keychain override and inspect effective non-secret metadata, but **never reads
//! a full secret back** across IPC.

use repodesk_core::credentials::{
    self, ANTHROPIC_API_KEY, CredentialResolver, EffectiveCredentialMetadata, GEMINI_API_KEY,
    KeyringResolver, OPENAI_API_KEY,
};
use repodesk_core::errors::{RepoDeskError, RepoDeskResult};

use crate::store::ProviderPreferences;

use super::ErrorPayload;

/// Keys the desktop is allowed to manage — guards against arbitrary keychain
/// writes from the webview.
const ALLOWED_KEYS: [&str; 3] = [OPENAI_API_KEY, ANTHROPIC_API_KEY, GEMINI_API_KEY];

fn ensure_allowed(key: &str) -> Result<(), ErrorPayload> {
    if ALLOWED_KEYS.contains(&key) {
        Ok(())
    } else {
        Err(ErrorPayload::from(format!(
            "unknown credential key '{key}'"
        )))
    }
}

/// Resolve the environment-variable name that belongs to a canonical credential
/// key. OpenAI/Gemini names are user-configurable preferences; Anthropic keeps
/// its canonical env name because there is no corresponding preference yet.
pub(crate) fn configured_env_var<'a>(
    preferences: &'a ProviderPreferences,
    key: &str,
) -> Option<&'a str> {
    let name = match key {
        OPENAI_API_KEY => preferences.openai_api_key_env_var.trim(),
        GEMINI_API_KEY => preferences.gemini_api_key_env_var.trim(),
        ANTHROPIC_API_KEY => "ANTHROPIC_API_KEY",
        _ => return None,
    };

    (!name.is_empty()).then_some(name)
}

pub(crate) fn configured_env_value<F>(
    preferences: &ProviderPreferences,
    key: &str,
    read_env: &F,
) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    configured_env_var(preferences, key)
        .and_then(read_env)
        .filter(|value| !value.trim().is_empty())
}

/// Read-only environment resolver that follows the same provider preference
/// names as Model Health and routing. This keeps auth provenance consistent
/// across Settings, diagnostics and execution planning.
pub(crate) struct ConfiguredEnvResolver<'a> {
    preferences: &'a ProviderPreferences,
}

impl<'a> ConfiguredEnvResolver<'a> {
    pub(crate) fn new(preferences: &'a ProviderPreferences) -> Self {
        Self { preferences }
    }
}

impl CredentialResolver for ConfiguredEnvResolver<'_> {
    fn get(&self, key: &str) -> RepoDeskResult<Option<String>> {
        Ok(configured_env_value(self.preferences, key, &|name| {
            std::env::var(name).ok()
        }))
    }

    fn set(&self, _key: &str, _value: &str) -> RepoDeskResult<()> {
        Err(RepoDeskError::Api(
            "the configured environment credential resolver is read-only".to_string(),
        ))
    }

    fn delete(&self, _key: &str) -> RepoDeskResult<()> {
        Err(RepoDeskError::Api(
            "the configured environment credential resolver is read-only".to_string(),
        ))
    }
}

fn credential_set_with_resolvers(
    key: &str,
    value: &str,
    keychain: &dyn CredentialResolver,
    environment: &dyn CredentialResolver,
) -> Result<EffectiveCredentialMetadata, ErrorPayload> {
    ensure_allowed(key)?;
    if value.trim().is_empty() {
        return Err(ErrorPayload::from(
            "credential value cannot be blank; use credential_delete to remove a keychain override",
        ));
    }

    keychain.set(key, value)?;
    Ok(credentials::effective_credential_metadata(
        keychain,
        environment,
        key,
    )?)
}

fn credential_delete_with_resolvers(
    key: &str,
    keychain: &dyn CredentialResolver,
    environment: &dyn CredentialResolver,
) -> Result<EffectiveCredentialMetadata, ErrorPayload> {
    ensure_allowed(key)?;
    keychain.delete(key)?;
    Ok(credentials::effective_credential_metadata(
        keychain,
        environment,
        key,
    )?)
}

fn credential_status_with_resolvers(
    keychain: &dyn CredentialResolver,
    environment: &dyn CredentialResolver,
) -> Result<Vec<EffectiveCredentialMetadata>, ErrorPayload> {
    ALLOWED_KEYS
        .into_iter()
        .map(|key| {
            credentials::effective_credential_metadata(keychain, environment, key)
                .map_err(ErrorPayload::from)
        })
        .collect()
}

/// Store a provider secret in the OS keychain. Returns source-aware masked
/// metadata, never the value. Blank input is rejected so deletion has one API.
#[tauri::command]
pub fn credential_set(
    key: String,
    value: String,
) -> Result<EffectiveCredentialMetadata, ErrorPayload> {
    let preferences = crate::store::read_provider_preferences()?;
    let keychain = KeyringResolver::new();
    let environment = ConfiguredEnvResolver::new(&preferences);
    credential_set_with_resolvers(&key, &value, &keychain, &environment)
}

/// Remove only the keychain override. If a read-only environment fallback exists,
/// the returned effective metadata reports `environment` after the deletion.
#[tauri::command]
pub fn credential_delete(key: String) -> Result<EffectiveCredentialMetadata, ErrorPayload> {
    let preferences = crate::store::read_provider_preferences()?;
    let keychain = KeyringResolver::new();
    let environment = ConfiguredEnvResolver::new(&preferences);
    credential_delete_with_resolvers(&key, &keychain, &environment)
}

/// Non-secret, source-aware metadata for every managed credential.
#[tauri::command]
pub fn credential_status() -> Result<Vec<EffectiveCredentialMetadata>, ErrorPayload> {
    let preferences = crate::store::read_provider_preferences()?;
    let keychain = KeyringResolver::new();
    let environment = ConfiguredEnvResolver::new(&preferences);
    credential_status_with_resolvers(&keychain, &environment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodesk_core::credentials::CredentialSource;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemoryResolver(Mutex<HashMap<String, String>>);

    impl CredentialResolver for MemoryResolver {
        fn get(&self, key: &str) -> RepoDeskResult<Option<String>> {
            Ok(self.0.lock().expect("resolver lock").get(key).cloned())
        }

        fn set(&self, key: &str, value: &str) -> RepoDeskResult<()> {
            self.0
                .lock()
                .expect("resolver lock")
                .insert(key.to_string(), value.to_string());
            Ok(())
        }

        fn delete(&self, key: &str) -> RepoDeskResult<()> {
            self.0.lock().expect("resolver lock").remove(key);
            Ok(())
        }
    }

    #[test]
    fn blank_set_is_rejected_instead_of_becoming_a_second_delete_api() {
        let keychain = MemoryResolver::default();
        let environment = MemoryResolver::default();
        keychain
            .set(OPENAI_API_KEY, "fixture-aaaa")
            .expect("seed keychain");

        let result = credential_set_with_resolvers(OPENAI_API_KEY, "   ", &keychain, &environment);

        assert!(result.is_err());
        assert_eq!(
            keychain
                .get(OPENAI_API_KEY)
                .expect("read keychain")
                .as_deref(),
            Some("fixture-aaaa")
        );
    }

    #[test]
    fn set_returns_keychain_provenance() {
        let keychain = MemoryResolver::default();
        let environment = MemoryResolver::default();

        let metadata =
            credential_set_with_resolvers(OPENAI_API_KEY, "fixture-aaaa", &keychain, &environment)
                .expect("store keychain override");

        assert!(metadata.configured);
        assert_eq!(metadata.source, CredentialSource::Keychain);
        assert_eq!(metadata.hint, "••••aaaa");
    }

    #[test]
    fn deleting_keychain_override_reveals_environment_fallback() {
        let keychain = MemoryResolver::default();
        let environment = MemoryResolver::default();
        keychain
            .set(OPENAI_API_KEY, "fixture-aaaa")
            .expect("seed keychain");
        environment
            .set(OPENAI_API_KEY, "fixture-bbbb")
            .expect("seed environment");

        let metadata = credential_delete_with_resolvers(OPENAI_API_KEY, &keychain, &environment)
            .expect("delete keychain override");

        assert!(metadata.configured);
        assert_eq!(metadata.source, CredentialSource::Environment);
        assert_eq!(metadata.hint, "••••bbbb");
        assert_eq!(keychain.get(OPENAI_API_KEY).expect("read keychain"), None);
    }

    #[test]
    fn deleting_without_fallback_returns_none_source() {
        let keychain = MemoryResolver::default();
        let environment = MemoryResolver::default();
        keychain
            .set(OPENAI_API_KEY, "fixture-aaaa")
            .expect("seed keychain");

        let metadata = credential_delete_with_resolvers(OPENAI_API_KEY, &keychain, &environment)
            .expect("delete keychain override");

        assert!(!metadata.configured);
        assert_eq!(metadata.source, CredentialSource::None);
        assert_eq!(metadata.hint, "");
    }

    #[test]
    fn status_returns_all_allowed_keys_without_full_secrets() {
        let keychain = MemoryResolver::default();
        let environment = MemoryResolver::default();
        keychain
            .set(OPENAI_API_KEY, "fixture-aaaa")
            .expect("seed keychain");
        environment
            .set(ANTHROPIC_API_KEY, "fixture-bbbb")
            .expect("seed environment");

        let status =
            credential_status_with_resolvers(&keychain, &environment).expect("status resolves");
        let debug = format!("{status:?}");

        assert_eq!(status.len(), ALLOWED_KEYS.len());
        assert!(!debug.contains("fixture-aaaa"));
        assert!(!debug.contains("fixture-bbbb"));
    }

    #[test]
    fn configured_environment_uses_provider_preference_names() {
        let preferences = crate::store::ProviderPreferences {
            openai_api_key_env_var: "CUSTOM_OPENAI_KEY".into(),
            gemini_api_key_env_var: "CUSTOM_GEMINI_KEY".into(),
            ..crate::store::ProviderPreferences::default()
        };
        let read_env = |name: &str| match name {
            "CUSTOM_OPENAI_KEY" => Some("fixture-openai".to_string()),
            "CUSTOM_GEMINI_KEY" => Some("fixture-gemini".to_string()),
            "ANTHROPIC_API_KEY" => Some("fixture-anthropic".to_string()),
            _ => None,
        };

        assert_eq!(
            configured_env_value(&preferences, OPENAI_API_KEY, &read_env),
            Some("fixture-openai".to_string())
        );
        assert_eq!(
            configured_env_value(&preferences, GEMINI_API_KEY, &read_env),
            Some("fixture-gemini".to_string())
        );
        assert_eq!(
            configured_env_value(&preferences, ANTHROPIC_API_KEY, &read_env),
            Some("fixture-anthropic".to_string())
        );
    }
}
