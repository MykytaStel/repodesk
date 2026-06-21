//! The six canonical phases of a RepoDesk task — the shared source of truth for
//! the workflow, rendered identically by the desktop Work tab and the CLI.
//!
//! Progression is **derived, not stored**: [`derive_progress`] is a pure
//! function over [`PhaseSignals`] (the same `*_ok` facts the workflow engine
//! already computes, plus orchestration/review/commit signals), so the phase a
//! task is in is always a deterministic function of reality and can never go
//! stale. The only persisted, user-controlled state is the [`ExecutionMode`]
//! (Agent run vs Manual handoff) chosen for the Execute phase.
//!
//! The state-machine discipline is the *gate*: the actionable phase is always
//! the first one whose predecessors are all done, so a user can never act on a
//! later phase before its prerequisites hold.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;
use crate::tasks::show_active_task;

/// One of the six task phases, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Pick a project, create a task, define the goal and bounds.
    Scope,
    /// Build bounded context, safety-scan, estimate cost, pick a route.
    Prepare,
    /// Run the change — RepoDesk launches an agent, or hands off a context pack.
    Execute,
    /// Inspect the agent's output: changed files, diff, cost, accept/reject.
    Review,
    /// Final verification: project checks, RepoPilot, secret scan, proof.
    Verify,
    /// Stage, commit, close the task, clean up the worktree, record the outcome.
    Finish,
}

impl Phase {
    /// All phases in progression order.
    pub const ALL: [Phase; 6] = [
        Phase::Scope,
        Phase::Prepare,
        Phase::Execute,
        Phase::Review,
        Phase::Verify,
        Phase::Finish,
    ];

    /// Zero-based position in the progression.
    pub fn index(self) -> usize {
        Phase::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }

    /// Stable machine slug (matches the serde representation).
    pub fn slug(self) -> &'static str {
        match self {
            Phase::Scope => "scope",
            Phase::Prepare => "prepare",
            Phase::Execute => "execute",
            Phase::Review => "review",
            Phase::Verify => "verify",
            Phase::Finish => "finish",
        }
    }

    /// Human-readable title.
    pub fn title(self) -> &'static str {
        match self {
            Phase::Scope => "Scope",
            Phase::Prepare => "Prepare",
            Phase::Execute => "Execute",
            Phase::Review => "Review",
            Phase::Verify => "Verify",
            Phase::Finish => "Finish",
        }
    }

    /// The next phase, or `None` past the end.
    pub fn next(self) -> Option<Phase> {
        Phase::ALL.get(self.index() + 1).copied()
    }
}

/// How the Execute phase runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// RepoDesk launches a coding-agent CLI inside an isolated worktree.
    #[default]
    AgentRun,
    /// RepoDesk generates a context pack the user copies to an external agent.
    ManualHandoff,
}

/// Status of one phase within the progression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseStatus {
    /// A predecessor is unfinished — not actionable yet.
    Locked,
    /// Actionable now, but no work has started.
    Available,
    /// Actionable now, with partial progress.
    InProgress,
    /// Every gate for this phase is satisfied.
    Done,
}

/// Pure inputs to [`derive_progress`]: the deterministic facts each phase gates
/// on. The desktop maps its `ProductWorkflowState` + latest orchestration run +
/// git state onto these; the CLI computes them from core services directly.
#[derive(Debug, Clone, Default)]
pub struct PhaseSignals {
    // ── Scope ────────────────────────────────────────────────────────────────
    pub project_ok: bool,
    pub task_ok: bool,
    pub goal_defined: bool,
    // ── Prepare ──────────────────────────────────────────────────────────────
    pub context_ok: bool,
    pub safety_ok: bool,
    pub route_ready: bool,
    pub cost_estimated: bool,
    /// Optional baseline checks ran before execution — distinct from the final
    /// verification in the Verify phase. Not required to leave Prepare.
    pub baseline_checks_ran: bool,
    // ── Execute ──────────────────────────────────────────────────────────────
    pub execution_started: bool,
    pub execution_succeeded: bool,
    // ── Review ───────────────────────────────────────────────────────────────
    pub has_changes: bool,
    pub changes_reviewed: bool,
    // ── Verify ───────────────────────────────────────────────────────────────
    /// Final verification passed (project checks + RepoPilot + secret scan).
    pub final_checks_ok: bool,
    // ── Finish ───────────────────────────────────────────────────────────────
    pub committed: bool,
}

impl PhaseSignals {
    /// Whether a given phase's gate is fully satisfied.
    fn is_done(&self, phase: Phase) -> bool {
        match phase {
            Phase::Scope => self.project_ok && self.task_ok && self.goal_defined,
            Phase::Prepare => self.context_ok && self.safety_ok && self.route_ready,
            Phase::Execute => self.execution_succeeded,
            // With no changes there is nothing to review; the phase is trivially
            // satisfied so the flow can advance to verification.
            Phase::Review => !self.has_changes || self.changes_reviewed,
            Phase::Verify => self.final_checks_ok,
            Phase::Finish => self.committed,
        }
    }

    /// Whether a phase has partial (but incomplete) progress.
    fn in_progress(&self, phase: Phase) -> bool {
        match phase {
            Phase::Scope => self.project_ok || self.task_ok,
            Phase::Prepare => {
                self.context_ok || self.safety_ok || self.cost_estimated || self.baseline_checks_ran
            }
            Phase::Execute => self.execution_started,
            Phase::Review => self.has_changes,
            Phase::Verify => false,
            Phase::Finish => false,
        }
    }
}

/// One phase, ready to render: its status and a one-line summary of what it does
/// or what is needed next.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseView {
    pub phase: Phase,
    pub status: PhaseStatus,
    pub title: String,
    pub summary: String,
}

/// The single primary call-to-action for the actionable phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseCta {
    pub phase: Phase,
    pub label: String,
    /// The desktop action id that fulfills this CTA, when one applies.
    pub action_id: Option<String>,
}

/// Everything the Work surface needs: the six phases with statuses, the single
/// actionable phase, the one primary CTA, and the chosen execution mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseProgress {
    pub phases: Vec<PhaseView>,
    pub current: Phase,
    pub cta: PhaseCta,
    pub execution_mode: ExecutionMode,
    /// True once every phase (through Finish) is done.
    pub complete: bool,
}

/// Derive the full phase progression from signals. Pure and deterministic.
pub fn derive_progress(signals: &PhaseSignals, mode: ExecutionMode) -> PhaseProgress {
    // The actionable phase is the first one not yet done; if all are done, the
    // flow is complete and the cursor rests on Finish.
    let current = Phase::ALL
        .iter()
        .copied()
        .find(|phase| !signals.is_done(*phase))
        .unwrap_or(Phase::Finish);
    // Complete means every phase is done *in sequence* — not merely that
    // `is_done(Finish)` happens to hold (a later gate can be vacuously satisfied,
    // e.g. a clean tree, while an earlier phase like Review is still pending).
    let complete = Phase::ALL.iter().all(|phase| signals.is_done(*phase));

    let phases = Phase::ALL
        .iter()
        .copied()
        .map(|phase| {
            // Sequence governs display: every phase before the cursor is done,
            // the cursor itself is actionable, and everything after is locked —
            // even a phase whose gate is only vacuously satisfied (e.g. Review
            // with no changes) stays locked until the flow actually reaches it.
            let status = if complete || phase.index() < current.index() {
                PhaseStatus::Done
            } else if phase == current {
                if signals.in_progress(phase) {
                    PhaseStatus::InProgress
                } else {
                    PhaseStatus::Available
                }
            } else {
                PhaseStatus::Locked
            };
            PhaseView {
                phase,
                status,
                title: phase.title().to_string(),
                summary: phase_summary(phase, signals, mode),
            }
        })
        .collect();

    PhaseProgress {
        phases,
        current,
        cta: phase_cta(current, signals, mode, complete),
        execution_mode: mode,
        complete,
    }
}

/// A one-line description of the actionable next step within a phase.
fn phase_summary(phase: Phase, signals: &PhaseSignals, mode: ExecutionMode) -> String {
    match phase {
        Phase::Scope => {
            if !signals.project_ok {
                "Select or connect a project"
            } else if !signals.task_ok {
                "Create an active task"
            } else if !signals.goal_defined {
                "Define the task goal and allowed paths"
            } else {
                "Project, task, and goal are set"
            }
        }
        Phase::Prepare => {
            if !signals.context_ok {
                "Build bounded context for this task"
            } else if !signals.safety_ok {
                "Resolve safety findings before sending to an AI"
            } else if !signals.route_ready {
                "Pick a model/executor route"
            } else {
                "Context is built, scanned, and routed"
            }
        }
        Phase::Execute => match mode {
            ExecutionMode::AgentRun => "Launch the coding agent in an isolated worktree",
            ExecutionMode::ManualHandoff => "Generate a context pack to hand to an external agent",
        },
        Phase::Review => {
            if signals.has_changes {
                "Review changed files and accept or reject"
            } else {
                "No changes to review yet"
            }
        }
        Phase::Verify => "Run final project checks and verification",
        Phase::Finish => "Stage, commit, and close the task",
    }
    .to_string()
}

/// The single primary CTA for the actionable phase.
fn phase_cta(
    current: Phase,
    signals: &PhaseSignals,
    mode: ExecutionMode,
    complete: bool,
) -> PhaseCta {
    if complete {
        return PhaseCta {
            phase: Phase::Finish,
            label: "Task complete".to_string(),
            action_id: None,
        };
    }
    let (label, action_id): (&str, Option<&str>) = match current {
        Phase::Scope => {
            if !signals.project_ok {
                ("Add or select a project", None)
            } else if !signals.task_ok {
                ("Create a task", None)
            } else {
                ("Define the goal", None)
            }
        }
        Phase::Prepare => {
            if !signals.context_ok {
                ("Build context", Some("context-build"))
            } else if !signals.safety_ok {
                ("Scan context safety", Some("safety-scan-context"))
            } else {
                ("Pick a route", None)
            }
        }
        Phase::Execute => match mode {
            ExecutionMode::AgentRun => ("Run agent", None),
            ExecutionMode::ManualHandoff => ("Generate context pack", Some("prompt-all")),
        },
        Phase::Review => ("Review diff", None),
        Phase::Verify => ("Run checks", Some("checks-run")),
        Phase::Finish => ("Commit changes", None),
    };
    PhaseCta {
        phase: current,
        label: label.to_string(),
        action_id: action_id.map(str::to_string),
    }
}

// ── Persisted state (user-controlled transitions) ────────────────────────────

/// The user-controlled, persisted phase state for a task. Everything else is
/// derived from signals; these are the explicit decisions a user makes that
/// reality can't infer: the chosen execution mode, and the acknowledgements that
/// a specific run's changes were reviewed / committed.
///
/// The review/commit acks are **keyed to the run id** they applied to, so a
/// later run automatically invalidates a stale ack (a new run id won't match) —
/// the flow re-opens Review/Finish for the new work without any reset bookkeeping.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskPhaseState {
    pub execution_mode: ExecutionMode,
    /// The run whose changeset the user accepted/rejected (reviewed).
    #[serde(default)]
    pub reviewed_run_id: Option<String>,
    /// The run whose changes the user committed.
    #[serde(default)]
    pub committed_run_id: Option<String>,
}

fn phase_state_path() -> RepoDeskResult<PathBuf> {
    Ok(show_active_task()?.config.run_dir.join("phase-state.json"))
}

/// Load the active task's persisted phase state, defaulting when absent or
/// unreadable (a corrupt file must never break the Work surface).
pub fn load_phase_state() -> RepoDeskResult<TaskPhaseState> {
    let path = phase_state_path()?;
    if !path.exists() {
        return Ok(TaskPhaseState::default());
    }
    let state = std::fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default();
    Ok(state)
}

fn save_phase_state(state: &TaskPhaseState) -> RepoDeskResult<()> {
    let path = phase_state_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(state)?)?;
    Ok(())
}

/// Persist the chosen execution mode for the active task (preserving acks).
pub fn set_execution_mode(mode: ExecutionMode) -> RepoDeskResult<TaskPhaseState> {
    let mut state = load_phase_state()?;
    state.execution_mode = mode;
    save_phase_state(&state)?;
    Ok(state)
}

/// Record that `run_id`'s changeset has been reviewed (accepted or rejected).
pub fn mark_reviewed(run_id: &str) -> RepoDeskResult<TaskPhaseState> {
    let mut state = load_phase_state()?;
    state.reviewed_run_id = Some(run_id.to_string());
    save_phase_state(&state)?;
    Ok(state)
}

/// Record that `run_id`'s changes have been committed.
pub fn mark_committed(run_id: &str) -> RepoDeskResult<TaskPhaseState> {
    let mut state = load_phase_state()?;
    state.committed_run_id = Some(run_id.to_string());
    save_phase_state(&state)?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_signals() -> PhaseSignals {
        PhaseSignals {
            project_ok: true,
            task_ok: true,
            goal_defined: true,
            context_ok: true,
            safety_ok: true,
            route_ready: true,
            cost_estimated: true,
            baseline_checks_ran: true,
            execution_started: true,
            execution_succeeded: true,
            has_changes: true,
            changes_reviewed: true,
            final_checks_ok: true,
            committed: true,
        }
    }

    #[test]
    fn phases_are_ordered_and_indexed() {
        assert_eq!(Phase::Scope.index(), 0);
        assert_eq!(Phase::Finish.index(), 5);
        assert_eq!(Phase::Scope.next(), Some(Phase::Prepare));
        assert_eq!(Phase::Finish.next(), None);
    }

    #[test]
    fn empty_signals_start_at_scope_with_single_cta() {
        let progress = derive_progress(&PhaseSignals::default(), ExecutionMode::AgentRun);
        assert_eq!(progress.current, Phase::Scope);
        assert!(!progress.complete);
        assert_eq!(progress.cta.phase, Phase::Scope);
        // Exactly one phase is actionable; the rest are locked.
        let locked = progress
            .phases
            .iter()
            .filter(|p| p.status == PhaseStatus::Locked)
            .count();
        assert_eq!(locked, 5);
    }

    /// Signals with Scope satisfied and nothing past it.
    fn scope_done() -> PhaseSignals {
        PhaseSignals {
            project_ok: true,
            task_ok: true,
            goal_defined: true,
            ..PhaseSignals::default()
        }
    }

    #[test]
    fn current_is_first_unfinished_phase() {
        // Scope satisfied → Prepare is current.
        let progress = derive_progress(&scope_done(), ExecutionMode::AgentRun);
        assert_eq!(progress.current, Phase::Prepare);
        assert_eq!(progress.phases[0].status, PhaseStatus::Done);
        assert_eq!(progress.phases[1].status, PhaseStatus::Available);
    }

    #[test]
    fn partial_prepare_is_in_progress() {
        let signals = PhaseSignals {
            context_ok: true, // started Prepare but not routed
            ..scope_done()
        };
        let progress = derive_progress(&signals, ExecutionMode::AgentRun);
        assert_eq!(progress.current, Phase::Prepare);
        assert_eq!(progress.phases[1].status, PhaseStatus::InProgress);
    }

    #[test]
    fn no_changes_makes_review_trivially_done() {
        let signals = PhaseSignals {
            has_changes: false,
            changes_reviewed: false,
            committed: false,
            ..full_signals()
        };
        let progress = derive_progress(&signals, ExecutionMode::AgentRun);
        // Review is skipped (done); Verify already ok → current is Finish.
        assert!(signals.is_done(Phase::Review));
        assert_eq!(progress.current, Phase::Finish);
    }

    #[test]
    fn later_vacuous_gate_does_not_mark_complete_while_review_pending() {
        // Changes exist and are unreviewed, but a clean-tree/commit signal is
        // (prematurely) set. The flow must rest at Review, not report complete.
        let signals = PhaseSignals {
            has_changes: true,
            changes_reviewed: false,
            final_checks_ok: true,
            committed: true,
            ..full_signals()
        };
        let progress = derive_progress(&signals, ExecutionMode::AgentRun);
        assert!(!progress.complete);
        assert_eq!(progress.current, Phase::Review);
        let finish = progress
            .phases
            .iter()
            .find(|p| p.phase == Phase::Finish)
            .unwrap();
        assert_eq!(finish.status, PhaseStatus::Locked);
    }

    #[test]
    fn fully_satisfied_signals_are_complete() {
        let progress = derive_progress(&full_signals(), ExecutionMode::AgentRun);
        assert!(progress.complete);
        assert_eq!(progress.current, Phase::Finish);
        assert!(
            progress
                .phases
                .iter()
                .all(|p| p.status == PhaseStatus::Done)
        );
        assert_eq!(progress.cta.label, "Task complete");
    }

    #[test]
    fn execution_mode_changes_execute_cta() {
        // Scope + Prepare satisfied → Execute is current.
        let signals = PhaseSignals {
            context_ok: true,
            safety_ok: true,
            route_ready: true,
            ..scope_done()
        };
        let agent = derive_progress(&signals, ExecutionMode::AgentRun);
        assert_eq!(agent.current, Phase::Execute);
        assert_eq!(agent.cta.label, "Run agent");

        let manual = derive_progress(&signals, ExecutionMode::ManualHandoff);
        assert_eq!(manual.cta.label, "Generate context pack");
        assert_eq!(manual.cta.action_id.as_deref(), Some("prompt-all"));
    }
}
