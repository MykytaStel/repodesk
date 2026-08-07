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
    pub change_governance: ChangeGovernanceSnapshot,
}

/// Deterministic, task-local engineering aggregate. During the RepoDesk 2
/// migration this registered IPC command also carries the two typed Work Item
/// mutations so we do not grow parallel transport plumbing faster than the
/// domain stabilizes. The frontend still exposes separate read/write functions;
/// validation, evidence and persistence remain in Rust core.
#[tauri::command]
pub fn work_engineering_intelligence(
    contract_update: Option<WorkItemContractUpdate>,
    scope_override_reason: Option<String>,
) -> Result<WorkEngineeringSnapshot, ErrorPayload> {
    if let Some(update) = contract_update {
        save_active_work_item_contract(update).map_err(ErrorPayload::from)?;
    }
    if let Some(reason) = scope_override_reason {
        record_active_scope_override(&reason).map_err(ErrorPayload::from)?;
    }

    let task = show_active_task().map_err(ErrorPayload::from)?;
    let intelligence =
        load_engineering_intelligence(&task.config.run_dir).map_err(ErrorPayload::from)?;
    let context_inspector =
        load_context_inspector(&task.config.run_dir).map_err(ErrorPayload::from)?;
    let work_item_contract = load_work_item_contract_snapshot(&task).map_err(ErrorPayload::from)?;
    let change_governance = load_active_change_governance().map_err(ErrorPayload::from)?;

    Ok(WorkEngineeringSnapshot {
        intelligence,
        context_inspector,
        work_item_contract,
        change_governance,
    })
}
