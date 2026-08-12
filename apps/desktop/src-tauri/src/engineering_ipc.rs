//! Thin desktop transport adapters for engineering-facing repository operations.
//!
//! This module owns IPC shape only. Filesystem safety, Git parsing and AI scan
//! policy remain in `repodesk-core`; frontend command names stay stable while
//! the desktop entrypoint sheds transitional inline modules.

use repodesk_core::code_workspace::{
    CodeWorkspaceFileStatus, load_active_code_workspace, read_active_code_document,
};
use serde_json::json;

#[tauri::command]
pub fn ai_discovery_scan() -> Result<repodesk_core::ai_discovery::AiDiscoveryReport, String> {
    repodesk_core::ai_discovery::write_ai_discovery_report().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn git_workspace_snapshot() -> Result<repodesk_core::git_workspace::GitWorkspaceSnapshot, String>
{
    Ok(repodesk_core::git_workspace::build_git_workspace_snapshot())
}

/// Unified diff for a single changed file in the active project. `cached`
/// selects the staged diff; the path is repo-relative and traversal-guarded in
/// core.
#[tauri::command]
pub fn git_file_diff(path: String, cached: bool) -> Result<String, String> {
    Ok(repodesk_core::git_workspace::active_file_diff(
        &path, cached,
    ))
}

/// Transitional compatibility projection used by the existing Changes and
/// RepoPilot hooks. The typed Code Workspace remains the owner of filesystem
/// safety and status parsing.
#[tauri::command]
pub fn code_workbench_snapshot() -> serde_json::Value {
    match load_active_code_workspace() {
        Ok(snapshot) => {
            let changed_files = snapshot
                .files
                .iter()
                .filter(|file| file.status != CodeWorkspaceFileStatus::Clean)
                .map(|file| file.path.clone())
                .collect::<Vec<_>>();
            json!({
                "connected": true,
                "changed_files": changed_files,
                "source": snapshot.source,
                "truncated": snapshot.truncated,
            })
        }
        Err(error) => json!({
            "connected": false,
            "error": error.to_string(),
            "changed_files": [],
        }),
    }
}

#[tauri::command]
pub fn read_code_file(relative_path: String) -> Result<serde_json::Value, String> {
    let document = read_active_code_document(&relative_path).map_err(|error| error.to_string())?;
    Ok(json!({
        "path": document.path,
        "bytes": document.bytes,
        "content": document.content,
        "language": document.language,
        "fingerprint": document.fingerprint,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engineering_transport_keeps_compatibility_command_shapes() {
        let _: fn() -> Result<repodesk_core::git_workspace::GitWorkspaceSnapshot, String> =
            git_workspace_snapshot;
        let _: fn(String, bool) -> Result<String, String> = git_file_diff;
        let _: fn() -> serde_json::Value = code_workbench_snapshot;
        let _: fn(String) -> Result<serde_json::Value, String> = read_code_file;
        let _: fn() -> Result<repodesk_core::ai_discovery::AiDiscoveryReport, String> =
            ai_discovery_scan;
    }
}
