use repodesk_core::code_drafts::{
    CodeDraftLoadInput, CodeDraftRecord, CodeDraftRecovery, CodeDraftSaveInput,
    delete_project_code_draft, load_project_code_draft, save_project_code_draft,
};
use repodesk_core::code_workspace::{
    CodeWorkspaceDocument, CodeWorkspaceSaveInput, CodeWorkspaceSaveResult, CodeWorkspaceSnapshot,
    load_active_code_workspace, read_active_code_document, save_active_code_document,
};
use repodesk_core::code_workspace_ops::{
    CodeWorkspaceCreateFileInput, CodeWorkspaceDeleteInput, CodeWorkspaceMutationResult,
    CodeWorkspaceRenameInput, create_active_code_directory, create_active_code_file,
    delete_active_code_path, rename_active_code_path,
};
use repodesk_core::code_workspace_search::{
    CodeProjectSearchInput, CodeProjectSearchResult, CodeQuickOpenResult,
    invalidate_active_quick_open_index, search_active_code_workspace, search_code_project,
};
use repodesk_core::projects::get_active_project;

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
    let result = save_active_code_document(input).map_err(|error| error.to_string())?;
    invalidate_active_quick_open_index();
    Ok(result)
}

#[tauri::command]
pub async fn code_workspace_draft_save(
    input: CodeDraftSaveInput,
) -> Result<CodeDraftRecord, String> {
    let project = get_active_project().map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || save_project_code_draft(project, input))
        .await
        .map_err(|error| format!("Draft save worker failed: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn code_workspace_draft_load(
    input: CodeDraftLoadInput,
) -> Result<Option<CodeDraftRecovery>, String> {
    let project = get_active_project().map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || load_project_code_draft(project, input))
        .await
        .map_err(|error| format!("Draft load worker failed: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn code_workspace_draft_delete(relative_path: String) -> Result<bool, String> {
    let project = get_active_project().map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || delete_project_code_draft(project, &relative_path))
        .await
        .map_err(|error| format!("Draft delete worker failed: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn code_workspace_quick_open(
    query: String,
    limit: Option<usize>,
) -> Result<Vec<CodeQuickOpenResult>, String> {
    search_active_code_workspace(&query, limit.unwrap_or(50)).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn code_workspace_project_search(
    input: CodeProjectSearchInput,
) -> Result<CodeProjectSearchResult, String> {
    let project = get_active_project().map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        search_code_project(&project.name, &project.path, input)
    })
    .await
    .map_err(|error| format!("Project search worker failed: {error}"))?
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn code_workspace_create_file(
    input: CodeWorkspaceCreateFileInput,
) -> Result<CodeWorkspaceMutationResult, String> {
    let result = create_active_code_file(input).map_err(|error| error.to_string())?;
    invalidate_active_quick_open_index();
    Ok(result)
}

#[tauri::command]
pub fn code_workspace_create_directory(
    relative_path: String,
) -> Result<CodeWorkspaceMutationResult, String> {
    let result = create_active_code_directory(&relative_path).map_err(|error| error.to_string())?;
    invalidate_active_quick_open_index();
    Ok(result)
}

#[tauri::command]
pub fn code_workspace_rename(
    input: CodeWorkspaceRenameInput,
) -> Result<CodeWorkspaceMutationResult, String> {
    let result = rename_active_code_path(input).map_err(|error| error.to_string())?;
    invalidate_active_quick_open_index();
    Ok(result)
}

#[tauri::command]
pub fn code_workspace_delete(
    input: CodeWorkspaceDeleteInput,
) -> Result<CodeWorkspaceMutationResult, String> {
    let result = delete_active_code_path(input).map_err(|error| error.to_string())?;
    invalidate_active_quick_open_index();
    Ok(result)
}
