use std::sync::LazyLock;

use repodesk_core::code_library::{
    CodeLibraryDefinition, CodeLibraryDocument, CodeLibraryGrantRequest, CodeLibraryRegistry,
    default_code_library_roots,
};
use repodesk_core::projects::get_active_project;

static CODE_LIBRARY_REGISTRY: LazyLock<CodeLibraryRegistry> =
    LazyLock::new(CodeLibraryRegistry::default);

pub fn issue_definition(
    project: &str,
    project_root: &std::path::Path,
    server_id: &str,
    uri: &str,
) -> Result<CodeLibraryDefinition, String> {
    CODE_LIBRARY_REGISTRY
        .issue_definition(CodeLibraryGrantRequest {
            project: project.to_string(),
            server_id: server_id.to_string(),
            project_root: project_root.to_path_buf(),
            uri: uri.to_string(),
            allowed_roots: default_code_library_roots(project_root),
            issued_at: chrono::Utc::now(),
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn code_library_read(handle: String) -> Result<CodeLibraryDocument, String> {
    let project = get_active_project().map_err(|error| error.to_string())?;
    CODE_LIBRARY_REGISTRY
        .read(&project.name, &handle)
        .map_err(|error| error.to_string())
}

pub fn clear_project(project: &str) {
    CODE_LIBRARY_REGISTRY.clear_project(project);
}
