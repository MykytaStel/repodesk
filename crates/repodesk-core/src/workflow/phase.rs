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

/// All the evidence the post-Prepare gates derive from. The Scope/Prepare facts
/// come from the workflow engine; the post-execution facts come **only** from the
/// run receipt and current git state — never a manual ack or a stray index entry.
/// Both the desktop and the CLI build this and call [`derive_signals`], so the
/// two surfaces can never drift apart.
#[derive(Debug, Clone, Default)]
pub struct Evidence {
    // Scope/Prepare facts (already computed by the workflow engine).
    pub project_ok: bool,
    pub task_ok: bool,
    pub goal_defined: bool,
    pub context_ok: bool,
    pub safety_ok: bool,
    pub route_ready: bool,
    pub cost_estimated: bool,
    pub baseline_checks_ran: bool,
    /// The chosen Execute mode.
    pub mode: ExecutionMode,
    /// The active task's run receipt, if a run has produced one.
    pub receipt: Option<crate::workflow::receipt::TaskRunReceipt>,
    /// Current HEAD and staged-index-tree shas (None when not a repo / no commit).
    pub head_sha: Option<String>,
    pub index_tree_sha: Option<String>,
    /// Whether the receipt's recorded finish commit still exists in the repo.
    pub finish_commit_exists: bool,
}

/// Turn evidence into the phase signals — the one place the "is this phase
/// really done?" rules live, so the desktop and CLI stay identical.
pub fn derive_signals(evidence: &Evidence) -> PhaseSignals {
    use crate::workflow::receipt::ReviewDecision;

    let receipt = evidence.receipt.as_ref();
    let execution = receipt.map(|r| &r.execution);
    let review = receipt.and_then(|r| r.review.as_ref());
    let verification = receipt.and_then(|r| r.verification.as_ref());
    let finish = receipt.and_then(|r| r.finish.as_ref());

    // A receipt is honored only when it was produced for the *current* execution
    // mode: an agent-run receipt can't satisfy a manual handoff, and a manual
    // import can't satisfy an agent run. This keeps the original invariant (a
    // stale run from the other mode never advances the flow) while letting a
    // manual-handoff import — which now writes a `ManualHandoff` receipt — count
    // as real execution evidence.
    let mode_matches = receipt
        .map(|r| r.execution_mode == evidence.mode)
        .unwrap_or(false);
    let execution_started = mode_matches;
    let rejected = review
        .map(|r| r.decision == ReviewDecision::Rejected)
        .unwrap_or(false);
    let execution_succeeded =
        mode_matches && execution.map(|e| e.succeeded()).unwrap_or(false) && !rejected;

    // The reviewable changeset is exactly the run's recorded files.
    let run_digest = execution.and_then(|e| e.changeset_digest.clone());
    let has_changes = run_digest.is_some();

    // Reviewed only when the human accepted *this* run's *exact* changeset.
    let changes_reviewed = matches!(
        (receipt, review),
        (Some(r), Some(rv))
            if rv.decision == ReviewDecision::Accepted
                && rv.run_id == r.run_id
                && Some(&rv.changeset_digest) == run_digest.as_ref()
    );

    // Verification counts only if it ran for this run and nothing has moved.
    let final_checks_ok = matches!((receipt, verification), (Some(r), Some(v))
    if v.run_id == r.run_id
        && match (evidence.head_sha.as_deref(), evidence.index_tree_sha.as_deref(), run_digest.as_deref()) {
            (Some(head), Some(tree), Some(digest)) => v.valid_for(head, tree, digest),
            _ => false,
        });

    // Committed only when a real commit landed for this run.
    let committed = matches!((receipt, finish), (Some(r), Some(f))
        if f.run_id == r.run_id && evidence.finish_commit_exists);

    // A recorded commit is terminal proof the whole chain held: the bounded
    // commit ([`super::finish::commit_reviewed_index`]) refuses unless the run
    // was accepted and verification was fresh. Committing then moves HEAD, which
    // would otherwise invalidate the now-historical verification — so a valid
    // finish marks the upstream phases done rather than re-opening Verify.
    let (execution_succeeded, changes_reviewed, final_checks_ok) = if committed {
        (true, true, true)
    } else {
        (execution_succeeded, changes_reviewed, final_checks_ok)
    };

    PhaseSignals {
        project_ok: evidence.project_ok,
        task_ok: evidence.task_ok,
        goal_defined: evidence.goal_defined,
        context_ok: evidence.context_ok,
        safety_ok: evidence.safety_ok,
        route_ready: evidence.route_ready,
        cost_estimated: evidence.cost_estimated,
        baseline_checks_ran: evidence.baseline_checks_ran,
        execution_started,
        execution_succeeded,
        has_changes,
        changes_reviewed,
        final_checks_ok,
        committed,
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
        // Verification is receipt-bound (run_id + HEAD + index tree + changeset),
        // so the surface drives a dedicated verify path, not the generic action.
        Phase::Verify => ("Run verification", None),
        Phase::Finish => ("Commit changes", None),
    };
    PhaseCta {
        phase: current,
        label: label.to_string(),
        action_id: action_id.map(str::to_string),
    }
}

// ── Persisted state (user-controlled transitions) ────────────────────────────

/// The user-controlled, persisted phase state for a task. The only decision
/// reality can't infer is the chosen Execute mode; everything past it (review /
/// verify / commit) is proven by the run receipt, not a stored acknowledgement.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskPhaseState {
    pub execution_mode: ExecutionMode,
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

/// Persist the chosen execution mode for the active task.
pub fn set_execution_mode(mode: ExecutionMode) -> RepoDeskResult<TaskPhaseState> {
    let mut state = load_phase_state()?;
    state.execution_mode = mode;
    save_phase_state(&state)?;
    Ok(state)
}

#[cfg(test)]
#[path = "phase_tests.rs"]
mod tests;
