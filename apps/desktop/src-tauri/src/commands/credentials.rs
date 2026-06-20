//! Desktop bridge for the OS-keychain credential store. The frontend can store,
//! delete, and check provider API keys, but **never reads a full secret back** —
//! only non-secret metadata (configured flag + masked hint) crosses the boundary.

use repodesk_core::credentials::{
    self, ANTHROPIC_API_KEY, CredentialMetadata, CredentialResolver, GEMINI_API_KEY,
    KeyringResolver, OPENAI_API_KEY,
};

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

/// Store a provider secret in the OS keychain. Returns masked metadata, never
/// the value. An empty value deletes the entry instead.
#[tauri::command]
pub fn credential_set(key: String, value: String) -> Result<CredentialMetadata, ErrorPayload> {
    ensure_allowed(&key)?;
    let resolver = KeyringResolver::new();
    if value.trim().is_empty() {
        resolver.delete(&key)?;
    } else {
        resolver.set(&key, &value)?;
    }
    Ok(resolver.metadata(&key)?)
}

/// Remove a provider secret from the OS keychain.
#[tauri::command]
pub fn credential_delete(key: String) -> Result<CredentialMetadata, ErrorPayload> {
    ensure_allowed(&key)?;
    let resolver = KeyringResolver::new();
    resolver.delete(&key)?;
    Ok(resolver.metadata(&key)?)
}

/// Non-secret metadata for every managed key: configured flag + masked hint.
/// `configured` reflects the keychain *or* an env-var fallback so the UI shows
/// the same source a run would use.
#[tauri::command]
pub fn credential_status() -> Result<Vec<CredentialMetadata>, ErrorPayload> {
    let mut out = Vec::with_capacity(ALLOWED_KEYS.len());
    for key in ALLOWED_KEYS {
        let resolved = credentials::resolve_secret(key);
        out.push(CredentialMetadata {
            key: key.to_string(),
            configured: resolved.as_deref().is_some_and(|v| !v.trim().is_empty()),
            hint: resolved
                .as_deref()
                .map(credentials::mask_hint)
                .unwrap_or_default(),
        });
    }
    Ok(out)
}
