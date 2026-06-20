//! Credential resolver: where RepoDesk reads/writes provider API keys.
//!
//! Secrets must never live in git, frontend localStorage, plaintext SQLite/TOML,
//! logs, or run artifacts (see `docs/SECURITY_MODEL.md`). This module gives the
//! core a small [`CredentialResolver`] interface so callers depend on the
//! abstraction, not on a storage detail. Two implementations ship:
//!
//! - [`KeyringResolver`] — the OS keychain (macOS Keychain, Windows Credential
//!   Manager, Linux Secret Service) via the `keyring` crate. The packaged app
//!   uses this.
//! - [`EnvResolver`] — reads environment variables, read-only. The dev fallback.
//!
//! Only non-secret *metadata* (configured flag + masked hint) is ever returned
//! to a UI; the full secret never leaves this layer toward the frontend.

use crate::errors::{RepoDeskError, RepoDeskResult};

/// Keychain service name under which all RepoDesk secrets are stored.
const SERVICE: &str = "repodesk";

/// Canonical credential keys. These are the only names RepoDesk stores; they are
/// not the same as a provider's *env var* (which `EnvResolver` reads).
pub const OPENAI_API_KEY: &str = "openai_api_key";
pub const ANTHROPIC_API_KEY: &str = "anthropic_api_key";
pub const GEMINI_API_KEY: &str = "gemini_api_key";

/// Non-secret metadata safe to hand to a UI.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CredentialMetadata {
    pub key: String,
    pub configured: bool,
    /// A masked hint like `••••1234`, never the full secret.
    pub hint: String,
}

/// A store of provider secrets. Implementations must never log or echo a secret.
pub trait CredentialResolver: Send + Sync {
    fn get(&self, key: &str) -> RepoDeskResult<Option<String>>;
    fn set(&self, key: &str, value: &str) -> RepoDeskResult<()>;
    fn delete(&self, key: &str) -> RepoDeskResult<()>;

    /// Non-secret metadata for `key` — configured flag + masked hint only.
    fn metadata(&self, key: &str) -> RepoDeskResult<CredentialMetadata> {
        let value = self.get(key)?;
        Ok(CredentialMetadata {
            key: key.to_string(),
            configured: value.as_deref().is_some_and(|v| !v.trim().is_empty()),
            hint: value.as_deref().map(mask_hint).unwrap_or_default(),
        })
    }
}

/// Mask a secret to a short, non-reversible hint: bullets plus the last 4 chars.
/// An empty or very short secret masks to bullets only.
pub fn mask_hint(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let visible: String = trimmed
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if trimmed.chars().count() <= 4 {
        "••••".to_string()
    } else {
        format!("••••{visible}")
    }
}

/// OS keychain-backed resolver (the packaged-app default).
pub struct KeyringResolver {
    service: String,
}

impl KeyringResolver {
    pub fn new() -> Self {
        Self {
            service: SERVICE.to_string(),
        }
    }

    fn entry(&self, key: &str) -> RepoDeskResult<keyring::Entry> {
        keyring::Entry::new(&self.service, key).map_err(map_keyring_error)
    }
}

impl Default for KeyringResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialResolver for KeyringResolver {
    fn get(&self, key: &str) -> RepoDeskResult<Option<String>> {
        match self.entry(key)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(map_keyring_error(error)),
        }
    }

    fn set(&self, key: &str, value: &str) -> RepoDeskResult<()> {
        self.entry(key)?
            .set_password(value)
            .map_err(map_keyring_error)
    }

    fn delete(&self, key: &str) -> RepoDeskResult<()> {
        match self.entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(map_keyring_error(error)),
        }
    }
}

/// Read-only resolver backed by environment variables. `key` is upper-cased
/// (`openai_api_key` → `OPENAI_API_KEY`). Writes are rejected — the dev fallback
/// is for reading existing env config, not persisting secrets.
pub struct EnvResolver;

impl CredentialResolver for EnvResolver {
    fn get(&self, key: &str) -> RepoDeskResult<Option<String>> {
        Ok(std::env::var(key.to_ascii_uppercase()).ok())
    }

    fn set(&self, _key: &str, _value: &str) -> RepoDeskResult<()> {
        Err(RepoDeskError::Api(
            "the environment-variable credential resolver is read-only".to_string(),
        ))
    }

    fn delete(&self, _key: &str) -> RepoDeskResult<()> {
        Err(RepoDeskError::Api(
            "the environment-variable credential resolver is read-only".to_string(),
        ))
    }
}

/// The default resolver for the packaged app: the OS keychain.
pub fn default_resolver() -> Box<dyn CredentialResolver> {
    Box::new(KeyringResolver::new())
}

/// Resolve a secret for use at call time: the keychain first, then an env-var
/// fallback so local development keeps working without storing anything.
pub fn resolve_secret(key: &str) -> Option<String> {
    if let Ok(Some(value)) = KeyringResolver::new().get(key)
        && !value.trim().is_empty()
    {
        return Some(value);
    }
    EnvResolver
        .get(key)
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
}

fn map_keyring_error(error: keyring::Error) -> RepoDeskError {
    // Never include the secret; keyring errors carry only platform/store detail.
    RepoDeskError::Api(format!("keychain error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// In-memory resolver so the trait + metadata logic can be tested without a
    /// real OS keychain (unavailable/headless in CI).
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

    #[test]
    fn mask_hint_shows_only_last_four() {
        assert_eq!(mask_hint(""), "");
        assert_eq!(mask_hint("abc"), "••••");
        assert_eq!(mask_hint("sk-secret-1234"), "••••1234");
    }

    #[test]
    fn resolver_round_trips_and_reports_metadata() {
        let resolver = MemoryResolver::default();
        // Unset → not configured, empty hint.
        let meta = resolver.metadata(OPENAI_API_KEY).unwrap();
        assert!(!meta.configured);
        assert_eq!(meta.hint, "");

        resolver.set(OPENAI_API_KEY, "sk-live-abcd9999").unwrap();
        assert_eq!(
            resolver.get(OPENAI_API_KEY).unwrap().as_deref(),
            Some("sk-live-abcd9999")
        );
        let meta = resolver.metadata(OPENAI_API_KEY).unwrap();
        assert!(meta.configured);
        assert_eq!(meta.hint, "••••9999");

        resolver.delete(OPENAI_API_KEY).unwrap();
        assert!(!resolver.metadata(OPENAI_API_KEY).unwrap().configured);
    }

    #[test]
    fn env_resolver_is_read_only() {
        assert!(EnvResolver.set("k", "v").is_err());
        assert!(EnvResolver.delete("k").is_err());
    }
}
