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

pub mod context;
pub mod plan;
pub mod runner;
pub mod types;

pub use plan::{available_capacities, build_plan, route_steps};
pub use runner::{RunOptions, load_latest_run, load_run, run_plan};
pub use types::{
    OrchestrationPlan, OrchestrationRun, RunStatus, SubAgentResult, SubAgentStatus, SubAgentTask,
    topological_order,
};
