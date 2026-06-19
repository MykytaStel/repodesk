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
pub use store::{confirm_outcome, list_outcomes, outcome_stats, record_run};
