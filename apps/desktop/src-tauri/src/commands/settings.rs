use super::ErrorPayload;
use crate::store;

#[tauri::command]
pub fn provider_preferences() -> Result<store::ProviderPreferences, ErrorPayload> {
    store::read_provider_preferences().map_err(ErrorPayload::from)
}

#[tauri::command]
pub async fn save_provider_preferences(
    input: store::ProviderPreferences,
) -> Result<store::ProviderPreferences, ErrorPayload> {
    store::save_provider_preferences(input).map_err(ErrorPayload::from)
}

#[tauri::command]
pub fn save_codex_quota_status(status: String) -> Result<store::ProviderPreferences, ErrorPayload> {
    let normalized = status.trim().to_ascii_lowercase();
    if !matches!(
        normalized.as_str(),
        "unknown" | "available" | "limited" | "empty"
    ) {
        return Err(ErrorPayload::configuration(
            "Codex quota status must be one of: unknown, available, limited, empty",
        ));
    }

    let mut preferences = store::read_provider_preferences()?;
    preferences.codex_quota_status = normalized;
    store::save_provider_preferences(preferences).map_err(ErrorPayload::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_preferences_shape_serializes_without_credentials() {
        let json = serde_json::to_string(&store::ProviderPreferences::default())
            .expect("serialize provider preferences");

        assert!(!json.contains("anthropic_api_key\""));
        assert!(!json.contains("openai_api_key\""));
        assert!(!json.contains("gemini_api_key\""));
    }
}
