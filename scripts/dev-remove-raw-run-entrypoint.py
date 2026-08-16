from pathlib import Path

path = Path("crates/repodesk-core/src/orchestrator/runner.rs")
text = path.read_text()

old = '''/// Execute `plan` with a fresh RepoDesk run identity.
pub async fn run_plan(
    plan: &OrchestrationPlan,
    opts: &RunOptions,
) -> RepoDeskResult<OrchestrationRun> {
    run_plan_with_id(plan, opts, reserve_run_id()).await
}

'''
if text.count(old) != 1:
    raise SystemExit(f"expected one raw fresh-run entrypoint, found {text.count(old)}")
text = text.replace(old, "", 1)

old_ref = "/// [`run_plan`] *before* any HTTP request or process launch."
new_ref = "/// the execution path *before* any HTTP request or process launch."
if text.count(old_ref) != 1:
    raise SystemExit(f"expected one stale run_plan doc link, found {text.count(old_ref)}")
text = text.replace(old_ref, new_ref, 1)

path.write_text(text)
