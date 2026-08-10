use std::sync::LazyLock;

use repodesk_core::language_tools::{
    LanguageToolInstallPreview, LanguageToolInstallResult, LanguageToolInstallService,
    LanguageToolInstallStatus,
};

pub(crate) static LANGUAGE_TOOL_INSTALLER: LazyLock<LanguageToolInstallService> =
    LazyLock::new(LanguageToolInstallService::default);

#[tauri::command]
pub fn language_tool_install_preview(
    recipe_id: String,
) -> Result<LanguageToolInstallPreview, String> {
    LANGUAGE_TOOL_INSTALLER
        .preview(&recipe_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn language_tool_install_confirm(
    confirmation_token: String,
) -> Result<LanguageToolInstallResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        LANGUAGE_TOOL_INSTALLER
            .install(&confirmation_token)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Language-tool install worker failed: {error}"))?
}

#[tauri::command]
pub fn language_tool_install_status(
    recipe_id: String,
) -> Result<Option<LanguageToolInstallStatus>, String> {
    LANGUAGE_TOOL_INSTALLER
        .status(&recipe_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn language_tool_install_cancel(recipe_id: String) -> Result<bool, String> {
    LANGUAGE_TOOL_INSTALLER
        .cancel(&recipe_id)
        .map_err(|error| error.to_string())
}
