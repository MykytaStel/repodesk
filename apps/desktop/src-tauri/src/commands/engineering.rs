use repodesk_core::engineering::{
    ContextInspectorReport, EngineeringIntelligence, WorkItemContractSnapshot,
    WorkItemContractUpdate, load_context_inspector, load_engineering_intelligence,
    load_work_item_contract_snapshot, save_active_work_item_contract,
};
use repodesk_core::tasks::show_active_task;
use serde::Serialize;

use super::ErrorPayload;

#[derive(Debug, Serialize)]
pub struct WorkEngineeringSnapshot {
    pub intelligence: EngineeringIntelligence,
    pub context_inspector: ContextInspectorReport,
    pub work_item_contract: WorkItemContractSnapshot,
}

/// Deterministic, task-local engineering read model. During the RepoDesk 2
/// migration this registered IPC command also accepts an optional typed contract
/// update so the UI can persist the new domain artifact without adding another
/// parallel desktop execution surface. Validation and persistence remain in core.
#[tauri::command]
pub fn work_engineering_intelligence(
    contract_update: Option<WorkItemContractUpdate>,
) -> Result<WorkEngineeringSnapshot, ErrorPayload> {
    if let Some(update) = contract_update {
        save_active_work_item_contract(update).map_err(ErrorPayload::from)?;
    }

    let task = show_active_task().map_err(ErrorPayload::from)?;
    let intelligence =
        load_engineering_intelligence(&task.config.run_dir).map_err(ErrorPayload::from)?;
    let context_inspector =
        load_context_inspector(&task.config.run_dir).map_err(ErrorPayload::from)?;
    let work_item_contract =
        load_work_item_contract_snapshot(&task).map_err(ErrorPayload::from)?;

    Ok(WorkEngineeringSnapshot {
        intelligence,
        context_inspector,
        work_item_contract,
    })
}
