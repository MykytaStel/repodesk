use repodesk_core::code_workspace::{
    CodeWorkspaceDocument, CodeWorkspaceSaveInput, CodeWorkspaceSaveResult, CodeWorkspaceSnapshot,
    load_active_code_workspace, read_active_code_document, save_active_code_document,
};

#[tauri::command]
pub fn code_workspace_snapshot() -> Result<CodeWorkspaceSnapshot, String> {
    load_active_code_workspace().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn code_workspace_read(relative_path: String) -> Result<CodeWorkspaceDocument, String> {
    read_active_code_document(&relative_path).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn code_workspace_save(
    input: CodeWorkspaceSaveInput,
) -> Result<CodeWorkspaceSaveResult, String> {
    save_active_code_document(input).map_err(|error| error.to_string())
}
