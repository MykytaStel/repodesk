use repodesk_core::engineering::{
    ContextInspectorReport, EngineeringIntelligence, load_context_inspector,
    load_engineering_intelligence,
};
use repodesk_core::tasks::show_active_task;

use super::ErrorPayload;

/// Deterministic, task-local Engineering Intelligence derived by replaying the
/// append-only engineering event ledger. The desktop never recomputes metrics;
/// it renders this core read model directly.
#[tauri::command]
pub fn work_engineering_intelligence() -> Result<EngineeringIntelligence, ErrorPayload> {
    let task = show_active_task().map_err(ErrorPayload::from)?;
    load_engineering_intelligence(&task.config.run_dir).map_err(ErrorPayload::from)
}

/// IDE-facing view of the active task's bounded context, including the latest
/// persisted manifest, compactness evidence, and changeset coverage.
#[tauri::command]
pub fn work_context_inspector() -> Result<ContextInspectorReport, ErrorPayload> {
    let task = show_active_task().map_err(ErrorPayload::from)?;
    load_context_inspector(&task.config.run_dir).map_err(ErrorPayload::from)
}
