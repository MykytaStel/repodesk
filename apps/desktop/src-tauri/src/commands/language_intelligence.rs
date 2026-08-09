use repodesk_core::language_intelligence::{
    LanguageIntelligenceSnapshot, active_language_intelligence_snapshot,
};
use tauri::{AppHandle, State};

use crate::language_server::{
    LanguageHover, LanguageLocation, LanguageServerManager, LanguageServerStatus, LanguageSymbol,
};

#[tauri::command]
pub fn language_intelligence_snapshot() -> Result<LanguageIntelligenceSnapshot, String> {
    active_language_intelligence_snapshot().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn language_server_status(
    manager: State<'_, LanguageServerManager>,
) -> Option<LanguageServerStatus> {
    manager.status()
}

#[tauri::command]
pub async fn language_server_sync_document(
    app: AppHandle,
    manager: State<'_, LanguageServerManager>,
    path: String,
    language: String,
    text: String,
) -> Result<LanguageServerStatus, String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        manager.sync_document(&app, &path, &language, &text)
    })
    .await
    .map_err(|error| format!("Language server worker failed: {error}"))?
}

#[tauri::command]
pub fn language_server_close_document(
    manager: State<'_, LanguageServerManager>,
    path: String,
) -> Result<(), String> {
    manager.close_document(&path)
}

#[tauri::command]
pub async fn language_server_hover(
    app: AppHandle,
    manager: State<'_, LanguageServerManager>,
    path: String,
    text: String,
    line: u32,
    column: u32,
) -> Result<Option<LanguageHover>, String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.hover(&app, &path, &text, line, column))
        .await
        .map_err(|error| format!("Language hover worker failed: {error}"))?
}

#[tauri::command]
pub async fn language_server_definition(
    app: AppHandle,
    manager: State<'_, LanguageServerManager>,
    path: String,
    text: String,
    line: u32,
    column: u32,
) -> Result<Vec<LanguageLocation>, String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        manager.definition(&app, &path, &text, line, column)
    })
    .await
    .map_err(|error| format!("Language definition worker failed: {error}"))?
}

#[tauri::command]
pub async fn language_server_references(
    app: AppHandle,
    manager: State<'_, LanguageServerManager>,
    path: String,
    text: String,
    line: u32,
    column: u32,
) -> Result<Vec<LanguageLocation>, String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        manager.references(&app, &path, &text, line, column)
    })
    .await
    .map_err(|error| format!("Language references worker failed: {error}"))?
}

#[tauri::command]
pub async fn language_server_document_symbols(
    app: AppHandle,
    manager: State<'_, LanguageServerManager>,
    path: String,
    text: String,
) -> Result<Vec<LanguageSymbol>, String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.document_symbols(&app, &path, &text))
        .await
        .map_err(|error| format!("Language symbols worker failed: {error}"))?
}

#[tauri::command]
pub async fn language_server_stop(
    manager: State<'_, LanguageServerManager>,
) -> Result<(), String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.stop())
        .await
        .map_err(|error| format!("Language server shutdown worker failed: {error}"))?;
    Ok(())
}
