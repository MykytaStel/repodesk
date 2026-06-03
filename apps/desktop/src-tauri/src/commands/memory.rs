use repodesk_core::memory::model::{MemoryEntry, MemoryProposal};
use repodesk_core::memory::{self, BrainLlm, ScanSummary, store};
use serde::Serialize;

use super::ErrorPayload;

const MAX_MEMORY_CONTENT: usize = 8_000;
const MAX_CAPTURE_TEXT: usize = 60_000;
const PREVIEW_BUDGET_TOKENS: usize = 1_500;

/// What the brain will inject into agent prompts, plus headline counts.
#[derive(Debug, Serialize)]
pub struct BrainPreview {
    pub markdown: String,
    pub estimated_tokens: usize,
    pub included: usize,
    pub excluded: usize,
    pub total_active: usize,
    pub pending_proposals: usize,
}

/// Build an Ollama client from saved provider settings (disabled if off/unset).
fn brain_llm() -> BrainLlm {
    match crate::store::read_provider_settings() {
        Ok(s) => BrainLlm::new(s.ollama_enabled, Some(s.ollama_url), Some(s.ollama_model)),
        Err(_) => BrainLlm::disabled(),
    }
}

fn check_len(value: &str, max: usize, label: &str) -> Result<(), ErrorPayload> {
    if value.len() > max {
        return Err(ErrorPayload::resource_limit(format!(
            "{label} is too large ({} > {max} bytes)",
            value.len()
        )));
    }
    Ok(())
}

// ── Entries ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn memory_add(
    project: String,
    content: String,
    category: String,
    tags: Vec<String>,
) -> Result<MemoryEntry, ErrorPayload> {
    check_len(&content, MAX_MEMORY_CONTENT, "content")?;
    Ok(store::add_memory(&project, &content, &category, &tags)?)
}

#[tauri::command]
pub async fn memory_list(project: String) -> Result<Vec<MemoryEntry>, ErrorPayload> {
    Ok(store::list_memory(&project)?)
}

#[tauri::command]
pub async fn memory_search(
    project: String,
    query: String,
) -> Result<Vec<MemoryEntry>, ErrorPayload> {
    Ok(store::search_entries(&project, &query)?)
}

#[tauri::command]
pub async fn memory_update(
    id: i64,
    content: String,
    category: String,
    tags: Vec<String>,
) -> Result<MemoryEntry, ErrorPayload> {
    check_len(&content, MAX_MEMORY_CONTENT, "content")?;
    Ok(store::update_entry(id, &content, &category, &tags)?)
}

#[tauri::command]
pub async fn memory_delete(id: i64) -> Result<(), ErrorPayload> {
    Ok(store::delete_entry(id)?)
}

#[tauri::command]
pub async fn memory_set_pinned(id: i64, pinned: bool) -> Result<(), ErrorPayload> {
    Ok(store::set_pinned(id, pinned)?)
}

#[tauri::command]
pub async fn memory_set_status(id: i64, status: String) -> Result<(), ErrorPayload> {
    if !matches!(status.as_str(), "active" | "archived" | "superseded") {
        return Err(ErrorPayload::configuration(
            "status must be active, archived, or superseded",
        ));
    }
    Ok(store::set_status(id, &status)?)
}

#[tauri::command]
pub async fn memory_consolidate(project: String) -> Result<String, ErrorPayload> {
    Ok(memory::consolidate_project_memory(&project)?)
}

#[tauri::command]
pub async fn memory_brain_preview(project: String) -> Result<BrainPreview, ErrorPayload> {
    let slice = memory::memory_slice(&project, PREVIEW_BUDGET_TOKENS)?;
    let pending = store::count_pending(&project)?;
    Ok(BrainPreview {
        markdown: slice.markdown,
        estimated_tokens: slice.estimated_tokens,
        included: slice.included_ids.len(),
        excluded: slice.excluded_ids.len(),
        total_active: slice.total_active,
        pending_proposals: pending,
    })
}

// ── Capture + proposals (human-approved) ─────────────────────────────────────

#[tauri::command]
pub async fn memory_capture(
    project: String,
    agent: String,
    text: String,
) -> Result<Vec<MemoryProposal>, ErrorPayload> {
    check_len(&text, MAX_CAPTURE_TEXT, "response text")?;
    let task_id = repodesk_core::tasks::show_active_task()
        .map(|t| t.config.id)
        .unwrap_or_default();
    let llm = brain_llm();
    Ok(memory::capture_from_text_smart(&project, &task_id, &agent, &text, &llm).await?)
}

#[tauri::command]
pub async fn memory_scan(project: String) -> Result<ScanSummary, ErrorPayload> {
    Ok(memory::scan(&project)?)
}

#[tauri::command]
pub async fn memory_proposals_list(
    project: String,
    all: bool,
) -> Result<Vec<MemoryProposal>, ErrorPayload> {
    let status = if all { None } else { Some("pending") };
    Ok(store::list_proposals(&project, status)?)
}

#[tauri::command]
pub async fn memory_proposal_accept(
    id: i64,
    keep_id: Option<i64>,
) -> Result<MemoryProposal, ErrorPayload> {
    Ok(memory::accept_proposal(id, keep_id)?)
}

#[tauri::command]
pub async fn memory_proposal_reject(id: i64) -> Result<MemoryProposal, ErrorPayload> {
    Ok(memory::reject_proposal(id)?)
}

#[tauri::command]
pub async fn memory_reconcile_conflict(id: i64) -> Result<MemoryProposal, ErrorPayload> {
    let llm = brain_llm();
    Ok(memory::reconcile_conflict(id, &llm).await?)
}
