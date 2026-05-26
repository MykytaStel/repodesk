use crate::store;

#[tauri::command]
pub fn provider_settings() -> Result<store::ProviderSettings, String> {
    store::read_provider_settings()
}

#[tauri::command]
pub async fn save_provider_settings(
    input: store::ProviderSettings,
) -> Result<store::ProviderSettings, String> {
    store::save_provider_settings(input)
}

#[tauri::command]
pub fn save_codex_quota_status(status: String) -> Result<store::ProviderSettings, String> {
    let normalized = status.trim().to_ascii_lowercase();
    if !matches!(
        normalized.as_str(),
        "unknown" | "available" | "limited" | "empty"
    ) {
        return Err(
            "Codex quota status must be one of: unknown, available, limited, empty".to_string(),
        );
    }

    let mut settings = store::read_provider_settings()?;
    settings.codex_quota_status = normalized;
    store::save_provider_settings(settings)
}
