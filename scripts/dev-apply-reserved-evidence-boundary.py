# Sequential retry after the earlier verified patch raced a separate branch-writing test workflow.
from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}: {old[:120]!r}")
    file.write_text(text.replace(old, new, 1))


evidence = "crates/repodesk-core/src/orchestrator/execution_evidence.rs"
replace_once(
    evidence,
    '''pub async fn run_plan(
    plan: &OrchestrationPlan,
    opts: &RunOptions,
) -> RepoDeskResult<OrchestrationRun> {
    let run = runner::run_plan(plan, opts).await?;

    if !run.dry_run {
        match finalize_execution_evidence(plan, &run) {
            Ok(state) if state.status == ExecutionEvidenceStatus::RecoveryRequired => {
                log_recovery_required(&run.run_id, state.detail.as_deref());
            }
            Ok(_) => {}
            Err(error) => {
                // Execution has already happened. Never turn this into an
                // execution failure; Review will fail closed because the receipt
                // is absent, while this warning preserves the persistence fault.
                log_recovery_required(&run.run_id, Some(&error.to_string()));
            }
        }
    }

    Ok(run)
}
''',
    '''pub async fn run_plan(
    plan: &OrchestrationPlan,
    opts: &RunOptions,
) -> RepoDeskResult<OrchestrationRun> {
    run_plan_with_id(plan, opts, runner::reserve_run_id()).await
}

/// Evidence-aware execution boundary for callers that must reserve the run id
/// before launch (for example strategy-selection telemetry). The reserved id
/// changes identity timing only; it must never bypass receipt finalization.
pub async fn run_plan_with_id(
    plan: &OrchestrationPlan,
    opts: &RunOptions,
    run_id: String,
) -> RepoDeskResult<OrchestrationRun> {
    let run = runner::run_plan_with_id(plan, opts, run_id).await?;
    finalize_after_execution(plan, run)
}

fn finalize_after_execution(
    plan: &OrchestrationPlan,
    run: OrchestrationRun,
) -> RepoDeskResult<OrchestrationRun> {
    if !run.dry_run {
        match finalize_execution_evidence(plan, &run) {
            Ok(state) if state.status == ExecutionEvidenceStatus::RecoveryRequired => {
                log_recovery_required(&run.run_id, state.detail.as_deref());
            }
            Ok(_) => {}
            Err(error) => {
                // Execution has already happened. Never turn this into an
                // execution failure; Review will fail closed because the receipt
                // is absent, while this warning preserves the persistence fault.
                log_recovery_required(&run.run_id, Some(&error.to_string()));
            }
        }
    }

    Ok(run)
}
''',
)

mod_rs = "crates/repodesk-core/src/orchestrator/mod.rs"
replace_once(
    mod_rs,
    '''pub use execution_evidence::{
    ExecutionEvidenceState, ExecutionEvidenceStatus, evidence_state_for_run,
    repair_execution_evidence, run_plan,
};
''',
    '''pub use execution_evidence::{
    ExecutionEvidenceState, ExecutionEvidenceStatus, evidence_state_for_run,
    repair_execution_evidence, run_plan, run_plan_with_id,
};
''',
)
replace_once(
    mod_rs,
    '''pub use runner::{
    AgentWorkspacePolicy, ExecutionAuthorization, RunOptions, list_runs, load_latest_run, load_run,
    reserve_run_id, run_plan_with_id,
};
''',
    '''pub use runner::{
    AgentWorkspacePolicy, ExecutionAuthorization, RunOptions, list_runs, load_latest_run, load_run,
    reserve_run_id,
};
''',
)

runner = "crates/repodesk-core/src/orchestrator/runner.rs"
replace_once(
    runner,
    '''pub async fn run_plan_with_id(
    plan: &OrchestrationPlan,
    opts: &RunOptions,
    run_id: String,
) -> RepoDeskResult<OrchestrationRun> {
''',
    '''pub(super) async fn run_plan_with_id(
    plan: &OrchestrationPlan,
    opts: &RunOptions,
    run_id: String,
) -> RepoDeskResult<OrchestrationRun> {
''',
)
