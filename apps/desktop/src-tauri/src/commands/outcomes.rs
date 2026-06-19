//! Desktop bridge for the orchestrator outcome ledger (N8-A) and the learned
//! routing bias it feeds (N8-B): list recent step outcomes, aggregate per-(task
//! kind, provider) stats for the active project, and confirm/override a verdict
//! (the human-in-the-loop knob). Recording happens in the runner; this is read +
//! the one human mutation.

use repodesk_core::outcomes::{self, OutcomeRecord, ProviderStat, Verdict};
use repodesk_core::projects::read_active_project;

use super::ErrorPayload;

/// Hard cap on how many outcome rows the UI can pull at once.
const MAX_OUTCOMES: usize = 200;

/// Recent step outcomes, newest first (the learning signal).
#[tauri::command]
pub async fn outcomes_list(limit: Option<usize>) -> Result<Vec<OutcomeRecord>, ErrorPayload> {
    let limit = limit.unwrap_or(50).clamp(1, MAX_OUTCOMES);
    Ok(outcomes::list_outcomes(limit)?)
}

/// Per-(task kind, provider) success/cost stats for the active project.
#[tauri::command]
pub async fn outcomes_stats() -> Result<Vec<ProviderStat>, ErrorPayload> {
    let project = read_active_project()?;
    Ok(outcomes::outcome_stats(&project)?)
}

/// Confirm or override the verdict for one outcome row (human-in-the-loop).
#[tauri::command]
pub async fn outcomes_confirm(id: i64, verdict: String) -> Result<(), ErrorPayload> {
    let parsed = match verdict.trim().to_ascii_lowercase().as_str() {
        "good" => Verdict::Good,
        "bad" => Verdict::Bad,
        "neutral" => Verdict::Neutral,
        other => {
            return Err(ErrorPayload::configuration(format!(
                "unknown verdict '{other}' — use good, bad, or neutral"
            )));
        }
    };
    outcomes::confirm_outcome(id, parsed)?;
    Ok(())
}
