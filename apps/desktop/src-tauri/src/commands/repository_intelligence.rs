#[tauri::command]
pub async fn repository_intelligence_snapshot(
    focus_path: Option<String>,
) -> Result<repodesk_core::repository_intelligence::RepositoryIntelligenceSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        repodesk_core::repository_intelligence::active_repository_intelligence(
            focus_path.as_deref(),
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Repository intelligence worker failed: {error}"))?
}
