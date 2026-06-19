//! Data model for the orchestrator outcome ledger.
//!
//! Each [`OutcomeRecord`] is one executed sub-agent step: what was routed where,
//! what it cost, and how it turned out. The [`Verdict`] starts as a provisional
//! `auto` judgement derived from the step status and only becomes authoritative
//! when a human confirms it — the same propose→approve discipline the Memory
//! Brain uses. [`ProviderStat`] is the aggregate the adaptive router consumes.

use serde::{Deserialize, Serialize};

use crate::orchestrator::types::SubAgentStatus;

/// How a step turned out, for learning purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The step succeeded — a positive signal for this (task_kind, provider).
    Good,
    /// The step failed or was blocked — a negative signal.
    Bad,
    /// No usable signal (skipped via an upstream failure, cost ceiling, manual
    /// route). Excluded from success-rate math so it can't skew learning.
    Neutral,
}

impl Verdict {
    /// The provisional verdict implied by a step's run status. Failures and
    /// blocks count against the provider; clean completions count for it; the
    /// rest carry no signal until a human says otherwise.
    pub fn auto_from_status(status: SubAgentStatus) -> Self {
        match status {
            SubAgentStatus::Ok => Verdict::Good,
            SubAgentStatus::Failed | SubAgentStatus::Blocked => Verdict::Bad,
            SubAgentStatus::Skipped => Verdict::Neutral,
        }
    }

    pub fn as_label(&self) -> &'static str {
        match self {
            Verdict::Good => "good",
            Verdict::Bad => "bad",
            Verdict::Neutral => "neutral",
        }
    }

    pub fn from_label(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "good" => Verdict::Good,
            "bad" => Verdict::Bad,
            _ => Verdict::Neutral,
        }
    }
}

/// One executed sub-agent step, as stored in the `run_outcomes` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeRecord {
    pub id: i64,
    pub created_at: String,
    pub project: String,
    pub task_id: String,
    pub run_id: String,
    pub step_id: String,
    /// Canonical `TaskKind` label (e.g. `plan`, `patch`, `review`).
    pub task_kind: String,
    pub provider: String,
    pub model: String,
    /// Canonical `SubAgentStatus` label (e.g. `ok`, `failed`).
    pub status: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cost_units: f64,
    pub verdict: Verdict,
    /// `auto` until a human confirms/overrides it, then `human`.
    pub verdict_source: String,
    pub confirmed: bool,
    pub notes: String,
}

/// Aggregate outcomes for one (task_kind, provider) pair — the fuel the adaptive
/// router (N8-B) reads. `success_rate` is `good / (good + bad)`; neutral rows are
/// excluded so skipped/manual steps don't distort it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStat {
    pub task_kind: String,
    pub provider: String,
    /// Rows that carry a signal (good + bad); neutral rows are not counted.
    pub scored_runs: usize,
    pub good: usize,
    pub bad: usize,
    pub neutral: usize,
    /// `good / (good + bad)`, or `None` when there is no scored signal yet.
    pub success_rate: Option<f64>,
    pub avg_cost_units: f64,
}
