use super::ErrorPayload;
use crate::store;
use crate::store::STORED_KEY_MASK;

/// Replace a stored key with the mask sentinel so the raw secret never leaves the
/// backend. An empty key stays empty (signals "not configured" to the UI).
fn mask(key: &str) -> String {
    if key.trim().is_empty() {
        String::new()
    } else {
        STORED_KEY_MASK.to_string()
    }
}

#[tauri::command]
pub fn provider_settings() -> Result<store::ProviderSettings, ErrorPayload> {
    let mut settings = store::read_provider_settings().map_err(ErrorPayload::from)?;
    settings.anthropic_api_key = mask(&settings.anthropic_api_key);
    settings.openai_api_key = mask(&settings.openai_api_key);
    settings.gemini_api_key = mask(&settings.gemini_api_key);
    Ok(settings)
}

#[tauri::command]
pub async fn save_provider_settings(
    mut input: store::ProviderSettings,
) -> Result<store::ProviderSettings, ErrorPayload> {
    // A key still equal to the mask means the user didn't retype it, so preserve
    // the previously stored secret rather than overwriting it with the mask.
    let existing = store::read_provider_settings().map_err(ErrorPayload::from)?;
    if input.anthropic_api_key == STORED_KEY_MASK {
        input.anthropic_api_key = existing.anthropic_api_key;
    }
    if input.openai_api_key == STORED_KEY_MASK {
        input.openai_api_key = existing.openai_api_key;
    }
    if input.gemini_api_key == STORED_KEY_MASK {
        input.gemini_api_key = existing.gemini_api_key;
    }
    let saved = store::save_provider_settings(input).map_err(ErrorPayload::from)?;
    // Re-mask before returning so the response never carries raw secrets either.
    Ok(store::ProviderSettings {
        anthropic_api_key: mask(&saved.anthropic_api_key),
        openai_api_key: mask(&saved.openai_api_key),
        gemini_api_key: mask(&saved.gemini_api_key),
        ..saved
    })
}

#[tauri::command]
pub fn save_codex_quota_status(status: String) -> Result<store::ProviderSettings, ErrorPayload> {
    let normalized = status.trim().to_ascii_lowercase();
    if !matches!(
        normalized.as_str(),
        "unknown" | "available" | "limited" | "empty"
    ) {
        return Err(ErrorPayload::configuration(
            "Codex quota status must be one of: unknown, available, limited, empty",
        ));
    }

    let mut settings = store::read_provider_settings()?;
    settings.codex_quota_status = normalized;
    store::save_provider_settings(settings).map_err(ErrorPayload::from)
}
