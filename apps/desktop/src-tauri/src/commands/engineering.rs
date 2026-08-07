use repodesk_core::engineering::{
    ChangeGovernanceSnapshot, ContextInspectorReport, EngineeringIntelligence,
    WorkItemContractSnapshot, WorkItemContractUpdate, load_active_change_governance,
    load_context_inspector, load_engineering_intelligence, load_work_item_contract_snapshot,
    record_active_scope_override, save_active_work_item_contract,
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
    let work_item_contract = load_work_item_contract_snapshot(&task).map_err(ErrorPayload::from)?;

    Ok(WorkEngineeringSnapshot {
        intelligence,
        context_inspector,
        work_item_contract,
    })
}

/// Latest ChangeSet governance replay for the active Work Item. This is a
/// read-only projection over the engineering ledger and Work Item Contract.
#[tauri::command]
pub fn work_change_governance() -> Result<ChangeGovernanceSnapshot, ErrorPayload> {
    load_active_change_governance().map_err(ErrorPayload::from)
}

/// Record an explicit one-ChangeSet human exception for a contract violation.
/// Core validates that the current gate is a scope violation; the UI cannot
/// manufacture an override for an unrelated or already-safe ChangeSet.
#[tauri::command]
pub fn work_record_scope_override(
    reason: String,
) -> Result<ChangeGovernanceSnapshot, ErrorPayload> {
    record_active_scope_override(&reason).map_err(ErrorPayload::from)
}
