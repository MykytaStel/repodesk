use repodesk_core::persistence::db::{add_memory, list_memory, MemoryEntry};

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
