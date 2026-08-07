use repodesk_core::engineering::{
    ContextInspectorReport, EngineeringIntelligence, load_context_inspector,
    load_engineering_intelligence,
};
use repodesk_core::tasks::show_active_task;
use serde::Serialize;

use super::ErrorPayload;

#[derive(Debug, Serialize)]
pub struct WorkEngineeringSnapshot {
    pub intelligence: EngineeringIntelligence,
    pub context_inspector: ContextInspectorReport,
}

/// Deterministic, task-local engineering read model. The desktop receives both
/// the factual Engineering Intelligence report and the IDE-facing Context
/// Inspector in one IPC call; React never replays the event ledger itself.
#[tauri::command]
pub fn work_engineering_intelligence() -> Result<WorkEngineeringSnapshot, ErrorPayload> {
    let task = show_active_task().map_err(ErrorPayload::from)?;
    let intelligence =
        load_engineering_intelligence(&task.config.run_dir).map_err(ErrorPayload::from)?;
    let context_inspector =
        load_context_inspector(&task.config.run_dir).map_err(ErrorPayload::from)?;

    Ok(WorkEngineeringSnapshot {
        intelligence,
        context_inspector,
    })
}
