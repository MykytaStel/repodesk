use repodesk_core::persistence::db::{MemoryEntry, add_memory, list_memory};

#[tauri::command]
pub async fn memory_add(
    project: String,
    content: String,
    category: String,
    tags: Vec<String>,
) -> Result<MemoryEntry, String> {
    add_memory(&project, &content, &category, &tags).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn memory_list(project: String) -> Result<Vec<MemoryEntry>, String> {
    list_memory(&project).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn memory_consolidate(project: String) -> Result<String, String> {
    repodesk_core::persistence::db::consolidate_project_memory(&project).map_err(|e| e.to_string())
}
