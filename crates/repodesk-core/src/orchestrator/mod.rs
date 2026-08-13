//! The orchestrator: turn the active task into a plan of sub-agent tasks, route
//! each to the cheapest capable model, and run them in dependency order — each
//! with its own bounded, Memory-Brain-injected context pack. Outputs flow to
//! dependents in-run and become human-reviewable Memory Brain proposals.
//!
//! Layout:
//! - [`types`]   — plan/result/run data model + topological ordering.
//! - [`plan`]    — deterministic decomposition, routed via [`crate::routing`].
//! - [`context`] — per-sub-agent context packs (reuses smart-context + brain).
//! - [`strategy`] — evidence-backed plan shaping layered over the stable planner.
//! - `runner`    — raw gated execution + run persistence (private boundary).
//! - `execution_evidence` — public execution boundary + receipt recovery state.

pub mod auto_loop;
pub mod context;
mod execution_evidence;
pub mod manual_import;
pub mod plan;
pub mod preview;
mod review;
mod review_evidence_gate;
mod review_transaction;
mod runner;
pub mod strategy;
pub mod strategy_preview;
pub mod types;

pub use auto_loop::{LoopIteration, LoopOptions, LoopRun, LoopStatus, run_loop};
pub use execution_evidence::{
    ExecutionEvidenceState, ExecutionEvidenceStatus, evidence_state_for_run,
    repair_execution_evidence, run_plan,
};
pub use manual_import::{ManualImport, ManualImportSource, import_manual_changes};
pub use plan::{
    available_capacities, build_plan, plan_has_coding_agent_step, plan_has_paid_provider_step,
    plan_has_paid_step, route_steps, step_uses_paid_provider,
};
pub use preview::{ExecutionPreview, ExecutionPreviewStep, execution_preview, preview_plan};
pub use review::{ReviewAction, ReviewedFile, RunReview, record_review};
pub use review_evidence_gate::review_run;
pub use runner::{
    AgentWorkspacePolicy, ExecutionAuthorization, RunOptions, list_runs, load_latest_run, load_run,
    reserve_run_id, run_plan_with_id,
};
pub use strategy::{build_strategy_plan, derive_active_ai_strategy};
pub use strategy_preview::{
    PreparedStrategyExecution, StrategyBaselineComparison, StrategyExecutionPreview,
    prepare_strategy_execution,
};
pub use types::{
    OrchestrationPlan, OrchestrationRun, RunStatus, RunSummary, SubAgentResult, SubAgentStatus,
    SubAgentTask, topological_order,
};
