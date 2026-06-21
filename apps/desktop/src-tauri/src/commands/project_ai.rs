use repodesk_core::project_ai_scan::{ProjectAiImportResult, ProjectAiScanReport};

use super::ErrorPayload;

/// Scan the active project for known AI instruction/config files.
#[tauri::command]
pub async fn project_ai_scan() -> Result<ProjectAiScanReport, ErrorPayload> {
    Ok(repodesk_core::project_ai_scan::scan_active_project_ai()?)
}

/// Import selected clean AI instruction files into the Memory Brain.
/// Empty `paths` means import every importable file from the scan.
#[tauri::command]
pub async fn project_ai_import(paths: Vec<String>) -> Result<ProjectAiImportResult, ErrorPayload> {
    Ok(repodesk_core::project_ai_scan::import_active_project_ai(
        paths,
    )?)
}
