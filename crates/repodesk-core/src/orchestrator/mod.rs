//! The orchestrator: turn the active task into a plan of sub-agent tasks, route
//! each to the cheapest capable model, and run them in dependency order — each
//! with its own bounded, Memory-Brain-injected context pack. Outputs flow to
//! dependents in-run and become human-reviewable Memory Brain proposals.
//!
//! Layout:
//! - [`types`]   — plan/result/run data model + topological ordering.
//! - [`plan`]    — deterministic decomposition, routed via [`crate::routing`].
//! - [`context`] — per-sub-agent context packs (reuses smart-context + brain).
//! - [`runner`]  — sequential, gated execution + run persistence.

pub mod auto_loop;
pub mod context;
pub mod plan;
pub mod review;
pub mod runner;
pub mod types;

pub use auto_loop::{LoopIteration, LoopOptions, LoopRun, LoopStatus, run_loop};
pub use plan::{
    available_capacities, build_plan, plan_has_coding_agent_step, plan_has_paid_provider_step,
    plan_has_paid_step, route_steps, step_uses_paid_provider,
};
pub use review::{ReviewAction, ReviewedFile, RunReview, record_review, review_run};
pub use runner::{
    AgentWorkspacePolicy, ExecutionAuthorization, RunOptions, list_runs, load_latest_run, load_run,
    run_plan,
};
pub use types::{
    OrchestrationPlan, OrchestrationRun, RunStatus, RunSummary, SubAgentResult, SubAgentStatus,
    SubAgentTask, topological_order,
};
