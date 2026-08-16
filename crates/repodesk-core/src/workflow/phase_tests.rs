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

// ── Evidence → signals (the receipt-bound gate) ─────────────────────────

use crate::orchestrator::types::{RunStatus, SubAgentStatus};
use crate::workflow::receipt::{
    ExecutionReceipt, ReviewDecision, ReviewReceipt, StepReceipt, TaskRunReceipt,
    VerificationReceipt, changeset_digest,
};

/// A receipt for an agent run with a prep step (Ok) and an implementation
/// step whose status + changed files are given.
fn run_receipt(run_id: &str, impl_status: SubAgentStatus, changed: &[&str]) -> TaskRunReceipt {
    let changed: Vec<String> = changed.iter().map(|s| s.to_string()).collect();
    let digest = if changed.is_empty() {
        None
    } else {
        Some(changeset_digest(&changed))
    };
    TaskRunReceipt {
        task_id: "t".into(),
        run_id: run_id.into(),
        execution_mode: ExecutionMode::AgentRun,
        base_commit: Some("base".into()),
        execution: ExecutionReceipt {
            status: RunStatus::Partial,
            required_steps: vec![
                StepReceipt {
                    task_id: "prep".into(),
                    status: SubAgentStatus::Ok,
                    allow_write: false,
                    changed_files: vec![],
                    change_evidence_status: crate::change_evidence::ChangeEvidenceStatus::Complete,
                    change_attribution: Default::default(),
                },
                StepReceipt {
                    task_id: "impl".into(),
                    status: impl_status,
                    allow_write: true,
                    changed_files: changed,
                    change_evidence_status: crate::change_evidence::ChangeEvidenceStatus::Complete,
                    change_attribution: Default::default(),
                },
            ],
            changeset_digest: digest,
        },
        review: None,
        verification: None,
        finish: None,
    }
}

fn evidence(receipt: TaskRunReceipt, mode: ExecutionMode) -> Evidence {
    Evidence {
        project_ok: true,
        task_ok: true,
        goal_defined: true,
        context_ok: true,
        safety_ok: true,
        route_ready: true,
        cost_estimated: true,
        baseline_checks_ran: false,
        mode,
        receipt: Some(receipt),
        head_sha: Some("head".into()),
        index_tree_sha: Some("tree".into()),
        finish_commit_exists: false,
    }
}

fn accepted(run_id: &str, paths: &[&str]) -> ReviewReceipt {
    let paths: Vec<String> = paths.iter().map(|s| s.to_string()).collect();
    ReviewReceipt {
        run_id: run_id.into(),
        decision: ReviewDecision::Accepted,
        changeset_digest: changeset_digest(&paths),
        reviewed_paths: paths,
        index_tree_after_accept: Some("tree".into()),
    }
}

#[test]
fn partial_run_with_failed_required_step_stays_in_execute() {
    let ev = evidence(
        run_receipt("r1", SubAgentStatus::Failed, &["src/a.rs"]),
        ExecutionMode::AgentRun,
    );
    let signals = derive_signals(&ev);
    assert!(!signals.execution_succeeded);
    assert_eq!(
        derive_progress(&signals, ExecutionMode::AgentRun).current,
        Phase::Execute
    );
}

#[test]
fn successful_run_without_accepted_review_stays_in_review() {
    // A stray staged file used to satisfy Review; now only an Accepted
    // receipt for this run's changeset can.
    let ev = evidence(
        run_receipt("r1", SubAgentStatus::Ok, &["src/a.rs"]),
        ExecutionMode::AgentRun,
    );
    let signals = derive_signals(&ev);
    assert!(signals.execution_succeeded);
    assert!(signals.has_changes);
    assert!(!signals.changes_reviewed);
    assert_eq!(
        derive_progress(&signals, ExecutionMode::AgentRun).current,
        Phase::Review
    );
}

#[test]
fn fresh_run_receipt_has_no_review_so_review_reopens() {
    // A new run overwrites the receipt with review = None (invalidating any
    // earlier accept), so Review re-opens for the new changeset.
    let ev = evidence(
        run_receipt("r2", SubAgentStatus::Ok, &["src/a.rs"]),
        ExecutionMode::AgentRun,
    );
    assert!(!derive_signals(&ev).changes_reviewed);
}

#[test]
fn review_digest_must_match_run_changeset() {
    // An Accepted review whose digest is for a different changeset (e.g. a
    // stale review) does not count for this run.
    let mut receipt = run_receipt("r1", SubAgentStatus::Ok, &["src/a.rs"]);
    receipt.review = Some(accepted("r1", &["src/OTHER.rs"]));
    let ev = evidence(receipt, ExecutionMode::AgentRun);
    assert!(!derive_signals(&ev).changes_reviewed);
}

#[test]
fn reject_returns_task_to_execute() {
    let mut receipt = run_receipt("r1", SubAgentStatus::Ok, &["src/a.rs"]);
    receipt.review = Some(ReviewReceipt {
        run_id: "r1".into(),
        decision: ReviewDecision::Rejected,
        changeset_digest: changeset_digest(&["src/a.rs".to_string()]),
        reviewed_paths: vec!["src/a.rs".into()],
        index_tree_after_accept: None,
    });
    let ev = evidence(receipt, ExecutionMode::AgentRun);
    let signals = derive_signals(&ev);
    assert!(!signals.execution_succeeded);
    assert_eq!(
        derive_progress(&signals, ExecutionMode::AgentRun).current,
        Phase::Execute
    );
}

#[test]
fn manual_import_receipt_advances_manual_mode_to_review() {
    // A manual handoff that has been imported writes a ManualHandoff receipt
    // with a changeset; in Manual mode it counts as real execution evidence,
    // so the flow advances to Review just like an agent run.
    let mut receipt = run_receipt("m1", SubAgentStatus::Ok, &["src/a.rs"]);
    receipt.execution_mode = ExecutionMode::ManualHandoff;
    let ev = evidence(receipt, ExecutionMode::ManualHandoff);
    let signals = derive_signals(&ev);
    assert!(signals.execution_succeeded);
    assert!(signals.has_changes);
    assert!(!signals.changes_reviewed);
    assert_eq!(
        derive_progress(&signals, ExecutionMode::ManualHandoff).current,
        Phase::Review
    );
}

#[test]
fn agent_receipt_never_satisfies_manual_mode_after_mode_switch() {
    // An agent-run receipt must not advance Manual mode even with the new
    // mode-matching gate — switching to Manual can't inherit an agent run.
    let receipt = run_receipt("r1", SubAgentStatus::Ok, &["src/a.rs"]);
    let ev = evidence(receipt, ExecutionMode::ManualHandoff);
    assert!(!derive_signals(&ev).execution_succeeded);
}

#[test]
fn manual_mode_never_inherits_an_agent_run() {
    // The same successful agent-run receipt, read in Manual mode, must not
    // advance past Execute (a manual handoff produces no run evidence).
    let ev = evidence(
        run_receipt("r1", SubAgentStatus::Ok, &["src/a.rs"]),
        ExecutionMode::ManualHandoff,
    );
    let signals = derive_signals(&ev);
    assert!(!signals.execution_succeeded);
    assert_eq!(
        derive_progress(&signals, ExecutionMode::ManualHandoff).current,
        Phase::Execute
    );
}

#[test]
fn verification_freshness_gates_verify_then_finish() {
    let digest = changeset_digest(&["src/a.rs".to_string()]);
    let mut receipt = run_receipt("r1", SubAgentStatus::Ok, &["src/a.rs"]);
    receipt.review = Some(accepted("r1", &["src/a.rs"]));

    // Accepted but unverified → Verify is current.
    let ev = evidence(receipt.clone(), ExecutionMode::AgentRun);
    let signals = derive_signals(&ev);
    assert!(signals.changes_reviewed);
    assert!(!signals.final_checks_ok);
    assert_eq!(
        derive_progress(&signals, ExecutionMode::AgentRun).current,
        Phase::Verify
    );

    // Fresh verification matching head/tree/digest → Verify done → Finish.
    receipt.verification = Some(VerificationReceipt {
        run_id: "r1".into(),
        head_sha: "head".into(),
        index_tree_sha: "tree".into(),
        changeset_digest: digest,
        commands: vec![],
        success: true,
        verified_at: "now".into(),
    });
    let ev_ok = evidence(receipt.clone(), ExecutionMode::AgentRun);
    assert!(derive_signals(&ev_ok).final_checks_ok);
    assert_eq!(
        derive_progress(&derive_signals(&ev_ok), ExecutionMode::AgentRun).current,
        Phase::Finish
    );

    // Staged index moved after checks → verification stale → Verify re-opens.
    let mut ev_stale = evidence(receipt, ExecutionMode::AgentRun);
    ev_stale.index_tree_sha = Some("tree-moved".into());
    assert!(!derive_signals(&ev_stale).final_checks_ok);
}
