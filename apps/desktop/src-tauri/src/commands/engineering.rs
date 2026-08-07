use repodesk_core::engineering::{EngineeringIntelligence, load_engineering_intelligence};
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
