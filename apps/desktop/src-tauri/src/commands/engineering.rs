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

/// Deterministic, task-local engineering read model. The desktop receives the
/// factual intelligence, context evidence, and Work Item contract in one IPC
/// call; React never replays the event ledger itself.
#[tauri::command]
pub fn work_engineering_intelligence() -> Result<WorkEngineeringSnapshot, ErrorPayload> {
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

#[tauri::command]
pub fn work_item_contract_save(
    input: WorkItemContractUpdate,
) -> Result<WorkItemContractSnapshot, ErrorPayload> {
    save_active_work_item_contract(input).map_err(ErrorPayload::from)
}
