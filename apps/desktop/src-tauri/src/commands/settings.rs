use super::ErrorPayload;
use crate::store;

#[tauri::command]
pub fn provider_settings() -> Result<store::ProviderSettings, ErrorPayload> {
    store::read_provider_settings().map_err(ErrorPayload::from)
}

#[tauri::command]
pub async fn save_provider_settings(
    input: store::ProviderSettings,
) -> Result<store::ProviderSettings, ErrorPayload> {
    store::save_provider_settings(input).map_err(ErrorPayload::from)
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
