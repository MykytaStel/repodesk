//! Orchestrator outcome ledger — the N8 "Hermes" learning signal.
//!
//! Every real (non-dry-run) orchestration step is recorded here: what was routed
//! where, what it cost, and how it turned out. The verdict is a provisional
//! `auto` judgement until a human confirms it, mirroring the Memory Brain's
//! propose→approve discipline. The adaptive router reads [`outcome_stats`]; this
//! module only records and aggregates — it never changes routing on its own.

pub mod model;
pub mod store;

pub use model::{OutcomeRecord, ProviderStat, Verdict};
pub use store::{confirm_outcome, list_outcomes, outcome_stats, routing_bias};

use crate::errors::RepoDeskResult;
use crate::orchestrator::types::{OrchestrationPlan, OrchestrationRun};

/// Record engineering execution telemetry first, then preserve the legacy
/// provider/model outcome ledger. Engineering instrumentation is best-effort and
/// intentionally includes dry runs; the legacy learning ledger still ignores
/// dry runs inside `store::record_run`.
pub fn record_run(plan: &OrchestrationPlan, run: &OrchestrationRun) -> RepoDeskResult<usize> {
    let _ = crate::engineering::instrumentation::record_orchestration_run(Some(plan), run);
    store::record_run(plan, run)
}
