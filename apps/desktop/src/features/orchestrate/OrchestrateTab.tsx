import { useEffect, useState } from "react";
import { useOrchestrate } from "./useOrchestrate";
import { useModels } from "../models/useModels";
import {
  planHasCodingAgentStep,
  planHasPaidStep,
  stepIsApprovalGated,
  type ExecutorAvailability,
  type CheckProof,
  type LoopRun,
  type LoopStatus,
  type OrchestrationPlan,
  type OrchestrationRun,
  type ReviewAction,
  type RunDiff,
  type RunReview,
  type RunSummary,
  type RunWorktreeStatus,
  type SubAgentResult,
  type SubAgentStatus,
  type TaskEvent,
} from "../../shared/api/orchestrate";
import { MetricCard, errorToMessage, formatCost, formatNumber, EmptyState, ActorBadge, DiffView } from "../../shared/ui/SharedComponents";
import type { TabId } from "../../shared/types/api";

const STATUS_COLOR: Record<SubAgentStatus, string> = {
  ok: "#3fb950",
  skipped: "#8b949e",
  blocked: "#d29922",
  failed: "#f85149",
};

function StatusBadge({ status }: { status: SubAgentStatus }) {
  return (
    <span
      style={{
        color: STATUS_COLOR[status] ?? "#8b949e",
        border: `1px solid ${STATUS_COLOR[status] ?? "#8b949e"}`,
        borderRadius: 6,
        padding: "1px 8px",
        fontSize: 12,
        textTransform: "uppercase",
        letterSpacing: 0.4,
      }}
    >
      {status}
    </span>
  );
}

function PlanPanel({ plan }: { plan: OrchestrationPlan }) {
  return (
    <section className="panel wide-panel orchestrate-plan">
      <div className="panel-title-row compact">
        <div>
          <p className="eyebrow">Plan</p>
          <h2>{plan.goal}</h2>
        </div>
        <span className="pill neutral">
          {plan.steps.length} step{plan.steps.length === 1 ? "" : "s"}
        </span>
      </div>
      <div className="orchestrate-plan-grid">
        {plan.steps.map((step, index) => {
          const executor = step.executor_id ?? step.agent;
          const provider = step.provider_id ?? step.provider;
          const gated = stepIsApprovalGated(step);
          return (
            <article key={step.id} className={`orchestrate-step-card ${gated ? "gated" : "auto"}`}>
              <div className="orchestrate-step-head">
                <span className="orchestrate-step-index">{index + 1}</span>
                <div className="orchestrate-step-pills">
                  <span className={`pill ${gated ? "warn" : "ok"}`}>{gated ? "Approval" : "Auto"}</span>
                  {step.allow_write && <span className="pill warn">Writes</span>}
                </div>
              </div>
              <h3>{step.title}</h3>
              <p className="orchestrate-step-id">{step.id}</p>
              <div className="orchestrate-step-meta">
                <span>{executor}</span>
                {provider && provider !== executor ? <span>{provider}</span> : null}
                <span>{step.model ?? "default model"}</span>
              </div>
              <p className="orchestrate-step-rule">
                {step.allow_write ? <strong>Writes permitted</strong> : "Read-only"}.
                {step.depends_on.length > 0 && ` Depends on: ${step.depends_on.join(", ")}.`}
              </p>
            </article>
          );
        })}
      </div>
      <p className="muted" style={{ marginTop: 12, fontSize: 13 }}>
        Steps marked <em>Approval-gated</em> spend money or launch a CLI agent — RepoDesk pauses
        for your OK before running them. The rest run automatically.
      </p>
    </section>
  );
}

function ExecutorStatusPanel({
  executors,
  loading,
}: {
  executors: ExecutorAvailability[];
  loading: boolean;
}) {
  function authLabel(executor: ExecutorAvailability): string {
    if (executor.auth_status === "authenticated" || executor.authenticated === true) return "authenticated";
    if (executor.auth_status === "unauthenticated" || executor.authenticated === false) return "not authenticated";
    return "auth unknown";
  }

  return (
    <section className="panel">
      <div className="panel-title-row compact">
        <div>
          <p className="eyebrow">CLI agents</p>
          <h2>Executor availability</h2>
          <p className="muted" style={{ marginTop: 4, fontSize: 13 }}>Status of local binary coding agents (like Claude Code) on your PATH.</p>
        </div>
      </div>
      {loading ? (
        <p className="muted">Checking PATH...</p>
      ) : executors.length === 0 ? (
        <EmptyState message="No coding-agent executors are registered." />
      ) : (
        <div className="table-list" style={{ marginTop: 8 }}>
          {executors.map((executor) => (
            <div className="table-row flex-col items-start gap-sm" key={executor.executor_id}>
              <div className="w-full flex justify-between items-center">
                <strong>{executor.label}</strong>
                <span className={`pill ${executor.available ? "ok" : "warn"}`}>
                  {executor.available ? "available" : "missing"}
                </span>
              </div>
              <div className="row-meta">
                <span>{executor.binary}</span>
                <span>{executor.executable_path ?? executor.status}</span>
                {executor.version ? <span>{executor.version}</span> : null}
                <span>{authLabel(executor)}</span>
                {executor.auth_source ? <span>{executor.auth_source}</span> : null}
                {executor.auth_detail ? <span>{executor.auth_detail}</span> : null}
              </div>
              <p className="muted" style={{ fontSize: 12, margin: 0 }}>
                Launch still requires explicit approval for this run.
              </p>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function WorktreeRecoveryPanel({
  worktrees,
  loading,
  cleaningWorkspace,
  onCleanup,
}: {
  worktrees: RunWorktreeStatus[];
  loading: boolean;
  cleaningWorkspace?: string | null;
  onCleanup: (worktree: RunWorktreeStatus) => void;
}) {
  if (!loading && worktrees.length === 0) return null;

  return (
    <section className="panel wide-panel">
      <div className="panel-title-row compact">
        <div>
          <p className="eyebrow">Recovery</p>
          <h2>Isolated worktrees</h2>
        </div>
        {loading ? <span className="pill neutral">loading</span> : <span className="pill">{worktrees.length}</span>}
      </div>
      {worktrees.length === 0 ? (
        <p className="muted">Checking managed worktrees...</p>
      ) : (
        <div className="table-list" style={{ marginTop: 8 }}>
          {worktrees.map((worktree) => {
            const changedPreview = worktree.changed_files.slice(0, 6).join(", ");
            const extraChanged = worktree.changed_files.length - 6;
            const cleanupBusy = cleaningWorkspace === worktree.workspace_id;
            return (
              <div className="table-row flex-col items-start gap-sm" key={worktree.workspace_id}>
                <div className="w-full flex justify-between items-center">
                  <strong>
                    {worktree.run_id ?? "unknown run"}
                    {worktree.step_id ? ` / ${worktree.step_id}` : ""}
                  </strong>
                  <span className={`pill ${worktree.dirty ? "warn" : "ok"}`}>
                    {worktree.dirty ? "dirty" : "clean"}
                  </span>
                </div>
                <div className="row-meta">
                  <span>{worktree.workspace_id}</span>
                  <span>{worktree.git_tracked ? "git tracked" : "not tracked"}</span>
                  <span>{worktree.changed_files.length} changed</span>
                  {worktree.created_at ? <span>{fmtTime(worktree.created_at)}</span> : null}
                </div>
                <p className="muted" style={{ fontSize: 12, margin: 0 }}>
                  {worktree.path}
                </p>
                {worktree.changed_files.length > 0 ? (
                  <p className="muted" style={{ fontSize: 12, margin: 0 }}>
                    Changed files: {changedPreview}
                    {extraChanged > 0 ? `, +${extraChanged} more` : ""}
                  </p>
                ) : null}
                {worktree.warnings.map((warning) => (
                  <p className="muted" key={warning} style={{ fontSize: 12, margin: 0, color: "#d29922" }}>
                    {warning}
                  </p>
                ))}
                <div className="button-row compact-buttons">
                  <button
                    className="tiny-button"
                    onClick={() => onCleanup(worktree)}
                    disabled={!worktree.removable || cleanupBusy}
                    title={
                      worktree.removable
                        ? "Remove this managed worktree with git worktree remove --force"
                        : "This worktree is not removable from RepoDesk"
                    }
                  >
                    {cleanupBusy ? "Cleaning..." : "Cleanup worktree"}
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}

function RunPanel({
  run,
  onOpenMemory,
  reviewResult,
}: {
  run: OrchestrationRun;
  onOpenMemory?: () => void;
  reviewResult?: RunReview | null;
}) {
  const captured = run.results.reduce((sum, r) => sum + r.captured_proposals, 0);
  const changedCount = run.results.reduce((sum, r) => sum + (r.changed_files?.length ?? 0), 0);
  const isolatedChanged = run.results.some((r) => r.workspace && (r.changed_files?.length ?? 0) > 0);
  const workspaces = run.results
    .flatMap((r) => (r.workspace ? [r.workspace] : []))
    .filter((workspace, index, all) => all.findIndex((w) => w.workspace_id === workspace.workspace_id) === index);
  return (
    <section className="panel">
      <p className="eyebrow">
        Run {run.run_id} — {run.status}
        {run.dry_run ? " (dry run)" : ""}
      </p>
      <div className="card-row" style={{ marginBottom: 12 }}>
        <MetricCard label="Input tokens" value={formatNumber(run.total_input_tokens)} detail="across sub-agents" />
        <MetricCard label="Output tokens" value={formatNumber(run.total_output_tokens)} detail="across sub-agents" />
        <MetricCard label="Cost" value={formatCost(run.total_cost_units, "units")} detail={run.dry_run ? "projected" : "actual"} />
      </div>
      <div className="table-list">
        {run.results.map((result: SubAgentResult) => (
          <div className="table-row flex-col items-start gap-sm" key={result.task_id} style={{ paddingBottom: 12 }}>
            <div className="w-full flex justify-between items-center">
              <strong>
                {result.task_id} — {result.agent !== result.provider ? `${result.agent} → ${result.provider}` : result.provider}
                {result.model ? ` / ${result.model}` : ""}
              </strong>
              <StatusBadge status={result.status} />
            </div>
            <div className="row-meta">
              <span>in {formatNumber(result.input_tokens)}</span>
              <span>out {formatNumber(result.output_tokens)}</span>
              <span>cost {formatCost(result.cost_units, "units")}</span>
              <span>captured {result.captured_proposals}</span>
              {result.changed_files && result.changed_files.length > 0 ? (
                <span>changed {result.changed_files.length}</span>
              ) : null}
            </div>
            {result.changed_files && result.changed_files.length > 0 ? (
              <p className="muted" style={{ fontSize: 12, margin: 0 }}>
                Changed files: {result.changed_files.join(", ")}
              </p>
            ) : null}
            {result.workspace ? (
              <p className="muted" style={{ fontSize: 12, margin: 0 }}>
                Workspace: {result.workspace.path}
              </p>
            ) : null}
            {result.notes.map((note, i) => (
              <p className="muted" key={i} style={{ fontSize: 12, margin: 0 }}>
                {note}
              </p>
            ))}
          </div>
        ))}
      </div>
      {!run.dry_run && (captured > 0 || changedCount > 0) && (
        <div className="button-row compact-buttons" style={{ marginTop: 12 }}>
          {captured > 0 && (
            <button className="tiny-button" onClick={onOpenMemory} disabled={!onOpenMemory}>
              Review {captured} captured memory proposal{captured === 1 ? "" : "s"}
            </button>
          )}
          {changedCount > 0 && <span className="pill warn">Review changes above</span>}
        </div>
      )}
      {changedCount > 0 && isolatedChanged && (
        <div className="notice warn" style={{ marginTop: 12 }}>
          Agent changes are in isolated worktree{workspaces.length === 1 ? "" : "s"}:{" "}
          {workspaces.map((workspace) => workspace.path).join(", ")}. Accept applies and stages them
          in the active checkout after conflict checks; Reject leaves the active checkout untouched.
        </div>
      )}
      {reviewResult && (
        <p className="muted" style={{ fontSize: 12, marginTop: 8 }}>
          {reviewResult.action === "accept" ? "Accepted" : "Reviewed"}:{" "}
          {reviewResult.processed.map((f) => `${f.path} (${f.outcome})`).join(", ") || "nothing"}
          {reviewResult.warnings.length > 0 ? ` — ${reviewResult.warnings.join("; ")}` : ""}
        </p>
      )}
    </section>
  );
}

function ReviewCenter({
  run,
  diffs,
  diffsLoading,
  proof,
  proofLoading,
  onRefreshProof,
  onRunChecks,
  onReview,
  reviewing,
  acceptingAndVerifying,
  reviewResult,
}: {
  run: OrchestrationRun;
  diffs: RunDiff[];
  diffsLoading: boolean;
  proof?: CheckProof | null;
  proofLoading: boolean;
  onRefreshProof: () => void;
  onRunChecks: () => void;
  onReview: (action: ReviewAction) => void;
  reviewing?: boolean;
  acceptingAndVerifying?: boolean;
  reviewResult?: RunReview | null;
}) {
  const changedCount = run.results.reduce((sum, result) => sum + (result.changed_files?.length ?? 0), 0);
  if (changedCount === 0 && diffs.length === 0) return null;

  const isolated = run.results.some((result) => result.workspace && (result.changed_files?.length ?? 0) > 0);
  const proofTone =
    proof?.success === true ? "ok"
    : proof?.success === false ? "danger"
    : "warn";
  const proofLabel =
    proof?.success === true ? "Checks passed"
    : proof?.success === false ? "Checks failed"
    : proofLoading ? "Checking proof"
    : "Proof pending";
  const stepProofs = proof?.step_proofs?.filter((step) => step.changed_files.length > 0 || step.verification_notes.length > 0) ?? [];

  return (
    <section className="panel wide-panel changeset-review">
      <div className="panel-title-row">
        <div>
          <p className="eyebrow">Changeset review</p>
          <h2>{changedCount} agent-changed file{changedCount === 1 ? "" : "s"}</h2>
          <p className="muted" style={{ margin: "4px 0 0", fontSize: 13 }}>
            Review the captured diff and verification evidence before accepting anything into the active checkout.
          </p>
        </div>
        <div className="button-row compact-buttons">
          <ActorBadge mode="manual" />
          <span className={`pill ${proofTone}`}>{proofLabel}</span>
        </div>
      </div>

      {isolated && (
        <div className="notice warn" style={{ marginTop: 12 }}>
          Changes are still in isolated worktrees. Accept applies and stages them in the active checkout,
          then RepoDesk runs full active-checkout checks before you commit.
        </div>
      )}

      <div className="button-row compact-buttons" style={{ marginTop: 12 }}>
        <button className="tiny-button" onClick={onRefreshProof} disabled={proofLoading}>
          {proofLoading ? "Loading proof..." : "Load latest proof"}
        </button>
        <button className="tiny-button" onClick={onRunChecks} disabled={proofLoading}>
          {proofLoading ? "Running..." : "Run active-checkout checks"}
        </button>
        <button className="primary-button" onClick={() => onReview("accept")} disabled={reviewing || acceptingAndVerifying || proofLoading}>
          {reviewing
            ? "Accepting..."
            : acceptingAndVerifying
              ? "Running checks..."
              : `Accept & run checks (${changedCount})`}
        </button>
        <button className="ghost-button" onClick={() => onReview("reject")} disabled={reviewing || acceptingAndVerifying}>
          Reject changes
        </button>
      </div>

      {reviewResult && (
        <div className={`notice ${reviewResult.action === "accept" ? "ok" : "warn"}`} style={{ marginTop: 12 }}>
          {reviewResult.action === "accept" ? "Accepted" : "Reviewed"}{" "}
          {reviewResult.processed.map((file) => `${file.path} (${file.outcome})`).join(", ") || "nothing"}
          {reviewResult.warnings.length > 0 ? ` — ${reviewResult.warnings.join("; ")}` : ""}
          {reviewResult.action === "accept" && acceptingAndVerifying ? " Checks are running now." : ""}
        </div>
      )}

      {proof?.warnings?.map((warning) => (
        <div className="notice warn" key={warning} style={{ marginTop: 12 }}>{warning}</div>
      ))}

      {stepProofs.length > 0 && (
        <div className="table-list" style={{ marginTop: 12 }}>
          {stepProofs.map((step) => (
            <div className="table-row flex-col items-start gap-sm" key={step.task_id}>
              <div className="w-full flex justify-between items-center">
                <strong>{step.task_id}</strong>
                <span className="pill neutral">{step.status}</span>
              </div>
              {step.changed_files.length > 0 && (
                <p className="muted" style={{ margin: 0, fontSize: 12 }}>
                  Changed: {step.changed_files.join(", ")}
                </p>
              )}
              {step.verification_notes.map((note) => (
                <p className="muted" style={{ margin: 0, fontSize: 12 }} key={note}>{note}</p>
              ))}
            </div>
          ))}
        </div>
      )}

      {proof?.summary && (
        <details style={{ marginTop: 12 }}>
          <summary className="muted" style={{ cursor: "pointer" }}>Checks summary</summary>
          <pre className="code-panel compact" style={{ whiteSpace: "pre-wrap" }}>{proof.summary}</pre>
        </details>
      )}

      <div style={{ marginTop: 16 }}>
        {diffsLoading ? (
          <p className="muted">Loading captured diffs...</p>
        ) : diffs.length === 0 ? (
          <EmptyState message="No diff receipts were captured for this run." hint="The agent may only have produced text output or untracked/binary files." />
        ) : (
          diffs.map((diff) => (
            <div className="panel" key={`${run.run_id}-${diff.task_id}`} style={{ marginTop: 12 }}>
              <div className="panel-title-row compact">
                <div>
                  <p className="eyebrow">{diff.task_id}</p>
                  <h2>{diff.changed_files.length} file{diff.changed_files.length === 1 ? "" : "s"}</h2>
                  <p className="muted" style={{ margin: 0, fontSize: 12 }}>
                    {diff.provider}{diff.model ? ` / ${diff.model}` : ""}
                  </p>
                </div>
                <span className={`pill ${diff.exists ? "ok" : "warn"}`}>
                  {diff.exists ? "diff captured" : "no diff"}
                </span>
              </div>
              {diff.changed_files.length > 0 && (
                <p className="muted" style={{ fontSize: 12 }}>
                  {diff.changed_files.join(", ")}
                </p>
              )}
              {diff.warnings.map((warning) => (
                <div className="notice warn" key={warning}>{warning}</div>
              ))}
              {diff.diff.trim() ? <DiffView diff={diff.diff} /> : null}
              {diff.truncated && <p className="muted" style={{ fontSize: 12 }}>Diff was truncated for UI safety.</p>}
            </div>
          ))
        )}
      </div>
    </section>
  );
}

function fmtTime(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString();
}

const LOOP_TONE: Record<LoopStatus, "ok" | "warn" | "danger" | "accent"> = {
  succeeded: "ok",
  needs_approval: "warn",
  guardrail_blocked: "danger",
  exhausted: "warn",
  dry_run: "accent",
};

const LOOP_HINT: Record<LoopStatus, string> = {
  succeeded: "An attempt completed every step.",
  needs_approval: "The plan includes gated steps — enable the matching approvals to run it.",
  guardrail_blocked: "A safety/budget guardrail stopped the loop — resolve it, then re-run.",
  exhausted: "Out of attempts or budget before succeeding — raise the limits and re-run.",
  dry_run: "Preview only — nothing was executed.",
};

function LoopPanel({ loop }: { loop: LoopRun }) {
  const tone = LOOP_TONE[loop.status] ?? "neutral";
  return (
    <section className="panel">
      <div className="panel-title-row compact">
        <p className="eyebrow" style={{ margin: 0 }}>Autonomous loop</p>
        <span className={`pill ${tone}`}>{loop.status.replace("_", " ")}</span>
      </div>
      <p className="muted" style={{ marginTop: 0 }}>{LOOP_HINT[loop.status]}</p>
      <div className="table-list">
        {loop.iterations.map((iteration) => (
          <div className="table-row flex-col items-start gap-sm" key={iteration.index}>
            <div className="w-full flex justify-between items-center">
              <strong>
                Attempt {iteration.index + 1}
                {iteration.run_id ? ` — ${iteration.run_id}` : ""}
              </strong>
              <span className="pill">{iteration.run_status}</span>
            </div>
            <div className="row-meta">
              <span>cost {formatCost(iteration.cost_units, "units")}</span>
            </div>
            <p className="muted" style={{ fontSize: 12, margin: 0 }}>{iteration.note}</p>
          </div>
        ))}
      </div>
      <p className="muted" style={{ marginTop: 12, fontSize: 12 }}>
        Total {formatCost(loop.total_cost_units, "units")} over {loop.iterations.length} attempt
        {loop.iterations.length === 1 ? "" : "s"}.
      </p>
    </section>
  );
}

function HistoryPanel({
  runs,
  activeRunId,
  onSelect,
  busy,
}: {
  runs: RunSummary[];
  activeRunId?: string;
  onSelect: (runId: string) => void;
  busy: boolean;
}) {
  return (
    <section className="panel wide-panel">
      <p className="eyebrow">History</p>
      {runs.length === 0 ? (
        <EmptyState message="No runs yet for this task." hint="Preview a plan and run it to build history." />
      ) : (
        <div className="table-list" style={{ marginTop: 8 }}>
          {runs.map((run) => (
            <button
              key={run.run_id}
              className={`table-row file-row ${run.run_id === activeRunId ? "active" : ""}`}
              onClick={() => onSelect(run.run_id)}
              disabled={busy}
              style={{ textAlign: "left" }}
            >
              <span className="task-row-main">
                <strong>{run.goal || run.run_id}</strong>
                <small>
                  {run.run_id} · {fmtTime(run.started_at)} · {run.step_count} step
                  {run.step_count === 1 ? "" : "s"} · {formatCost(run.total_cost_units, "units")}
                </small>
              </span>
              <span className="file-badges">
                {run.dry_run && <span className="pill neutral">dry</span>}
                <span className="pill">{run.status}</span>
              </span>
            </button>
          ))}
        </div>
      )}
    </section>
  );
}

function TimelinePanel({ events }: { events: TaskEvent[] }) {
  if (events.length === 0) return null;
  return (
    <section className="panel wide-panel">
      <div className="panel-title-row compact">
        <div>
          <p className="eyebrow">Task activity</p>
          <p className="muted" style={{ margin: 0, fontSize: 13 }}>Real-time event log of internal backend operations</p>
        </div>
      </div>
      <div className="table-list" style={{ marginTop: 8 }}>
        {events.slice(0, 20).map((event, i) => (
          <div className="table-row flex-col items-start gap-sm" key={`${event.timestamp}-${i}`}>
            <div className="w-full flex justify-between items-center">
              <strong style={{ fontSize: 13 }}>{event.message}</strong>
              <span className="muted" style={{ fontSize: 12 }}>
                {fmtTime(event.timestamp)}
              </span>
            </div>
            <div className="row-meta">
              <span>{event.module_name}</span>
              <span>{event.level}</span>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

export function OrchestrateTab({ setActiveTab }: { setActiveTab?: (tab: TabId) => void }) {
  const orchestrate = useOrchestrate();
  const { models } = useModels();
  const [goal, setGoal] = useState("");
  const [maxCost, setMaxCost] = useState("");
  const [maxIterations, setMaxIterations] = useState("3");
  const [approvePaid, setApprovePaid] = useState(false);
  const [approveCodingAgents, setApproveCodingAgents] = useState(false);
  const [targetModelCombo, setTargetModelCombo] = useState<string>("auto");
  const [selectedRun, setSelectedRun] = useState<OrchestrationRun | null>(null);
  const [reviewResult, setReviewResult] = useState<RunReview | null>(null);
  const [acceptingAndVerifying, setAcceptingAndVerifying] = useState(false);
  const [cleaningWorkspace, setCleaningWorkspace] = useState<string | null>(null);

  const busy = orchestrate.plan.isPending || orchestrate.run.isPending || orchestrate.loop.isPending;
  // A run just executed wins; otherwise a history selection; otherwise the latest run.
  const currentRun = orchestrate.run.data ?? selectedRun ?? orchestrate.status;
  const error =
    orchestrate.run.error ??
    orchestrate.plan.error ??
    orchestrate.showRun.error ??
    orchestrate.loop.error ??
    orchestrate.runDiffs.error ??
    orchestrate.checkProof.error ??
    orchestrate.review.error ??
    orchestrate.cleanupWorktree.error;

  useEffect(() => {
    if (!currentRun || currentRun.dry_run) return;
    const hasChanges = currentRun.results.some((result) => (result.changed_files?.length ?? 0) > 0 || result.diff_path);
    if (!hasChanges) return;
    orchestrate.runDiffs.mutate(currentRun.run_id);
    orchestrate.checkProof.mutate({ runId: currentRun.run_id, runChecks: false });
  }, [currentRun?.run_id]);

  async function handleReview(action: ReviewAction) {
    if (!currentRun) return;
    setAcceptingAndVerifying(false);
    try {
      const result = await orchestrate.review.mutateAsync({ runId: currentRun.run_id, action });
      setReviewResult(result);
      if (action === "accept") {
        setAcceptingAndVerifying(true);
        await orchestrate.checkProof.mutateAsync({ runId: currentRun.run_id, runChecks: true });
      }
    } catch {
      // surfaced via orchestrate.review.error / the Error panel
    } finally {
      if (action === "accept") setAcceptingAndVerifying(false);
    }
  }

  async function handleRunChecksForCurrentRun() {
    if (!currentRun) return;
    await orchestrate.checkProof.mutateAsync({ runId: currentRun.run_id, runChecks: true });
  }

  async function handleCleanupWorktree(worktree: RunWorktreeStatus) {
    const changed = worktree.changed_files.length;
    const suffix = changed > 0 ? ` It has ${changed} changed file${changed === 1 ? "" : "s"}.` : "";
    const ok = window.confirm(`Remove worktree ${worktree.workspace_id}?${suffix}`);
    if (!ok) return;
    setCleaningWorkspace(worktree.workspace_id);
    try {
      await orchestrate.cleanupWorktree.mutateAsync(worktree.workspace_id);
    } finally {
      setCleaningWorkspace(null);
    }
  }

  function parsedMaxCost(): number | null {
    const parsed = maxCost.trim() ? Number(maxCost) : null;
    return Number.isFinite(parsed as number) ? parsed : null;
  }

  async function handleLoop(dryRun: boolean) {
    const overrideProvider = targetModelCombo === "auto" ? undefined : targetModelCombo.split("::")[0];
    const overrideModel = targetModelCombo === "auto" ? undefined : targetModelCombo.split("::").slice(1).join("::");
    await orchestrate.loop.mutateAsync({
      goal,
      maxIterations: Number(maxIterations) || 3,
      maxCost: parsedMaxCost(),
      dryRun,
      approvePaid,
      approveCodingAgents,
      overrideProvider,
      overrideModel,
    });
  }

  async function selectRun(runId: string) {
    const detail = await orchestrate.showRun.mutateAsync(runId);
    if (detail) setSelectedRun(detail);
  }

  async function handlePreview() {
    const overrideProvider = targetModelCombo === "auto" ? undefined : targetModelCombo.split("::")[0];
    const overrideModel = targetModelCombo === "auto" ? undefined : targetModelCombo.split("::").slice(1).join("::");
    await orchestrate.plan.mutateAsync({ goal, overrideProvider, overrideModel });
  }

  async function handleRun(dryRun: boolean) {
    const overrideProvider = targetModelCombo === "auto" ? undefined : targetModelCombo.split("::")[0];
    const overrideModel = targetModelCombo === "auto" ? undefined : targetModelCombo.split("::").slice(1).join("::");
    const built = await orchestrate.plan.mutateAsync({ goal, overrideProvider, overrideModel });
    const hasPaid = planHasPaidStep(built);
    const hasCodingAgent = planHasCodingAgentStep(built);
    if (!dryRun && hasCodingAgent && !approveCodingAgents) {
      window.alert("This plan includes coding-agent CLI steps. Enable approve CLI agents to launch them.");
      return;
    }
    if (!dryRun && (hasPaid || hasCodingAgent)) {
      const approvals = [
        hasPaid ? "paid provider steps that may spend tokens" : "",
        hasCodingAgent ? "local coding-agent CLI processes" : "",
      ].filter(Boolean);
      const ok = window.confirm(`This plan includes ${approvals.join(" and ")}. Run for real?`);
      if (!ok) return;
    }
    await orchestrate.run.mutateAsync({
      goal,
      dryRun,
      maxCost: parsedMaxCost(),
      approvePaid,
      approveCodingAgents,
      overrideProvider,
      overrideModel,
    });
  }

  if (!orchestrate.ready) {
    return (
      <div className="content-grid">
        <section className="hero-panel wide-panel">
          <p className="eyebrow">Orchestrator</p>
          <h1>No active project + task</h1>
          <p className="lead">
            Connect a project in Settings and create a task. The orchestrator turns the active
            task into a plan of sub-agents, each routed to the cheapest capable model.
          </p>
        </section>
      </div>
    );
  }

  return (
    <div className="content-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Orchestrator</p>
        <h1>
          Conduct sub-agents for{" "}
          <em style={{ fontStyle: "normal", color: "var(--accent)" }}>{orchestrate.projectName}</em>
        </h1>
        <p className="lead">
          Each sub-agent gets its own bounded, Memory-Brain-injected context and a per-task model
          (cheap/local where it can, premium where it must). Outputs flow downstream and become
          human-reviewable memory proposals. Preview the plan and cost before running.
        </p>

        <div className="orchestrate-control-panel">
          <label className="orchestrate-field orchestrate-goal-field">
            Goal override
            <textarea
              value={goal}
              onChange={(e) => setGoal(e.target.value)}
              placeholder="Leave empty to default to the active task title"
              rows={2}
            />
          </label>

          <label className="orchestrate-field">
            Target model
            <select
              value={targetModelCombo}
              onChange={(e) => setTargetModelCombo(e.target.value)}
            >
              <option value="auto">Adaptive Router (Auto)</option>
              {models?.providers.map((provider) => {
                // Fallback for CLI/local providers with no models
                if (provider.models.length === 0) {
                  return <option key={provider.id} value={`${provider.id}::`}>{provider.label}</option>;
                }
                return (
                  <optgroup key={provider.id} label={provider.label}>
                    {provider.models.map((m) => (
                      <option key={`${provider.id}::${m.id}`} value={`${provider.id}::${m.id}`}>
                        {m.id}
                      </option>
                    ))}
                  </optgroup>
                );
              })}
            </select>
          </label>

          <div className="orchestrate-limit-grid">
            <label className="orchestrate-field">
              Max cost
              <input
                value={maxCost}
                onChange={(e) => setMaxCost(e.target.value)}
                placeholder="No limit"
                inputMode="decimal"
              />
            </label>
            <label className="orchestrate-field">
              Loop attempts
              <input
                value={maxIterations}
                onChange={(e) => setMaxIterations(e.target.value)}
                inputMode="numeric"
              />
            </label>
          </div>

          <div className="orchestrate-approval-grid">
            <label className={`orchestrate-approval-card ${approvePaid ? "selected" : ""}`}>
              <input type="checkbox" checked={approvePaid} onChange={(e) => setApprovePaid(e.target.checked)} />
              <span>
                <strong>Paid steps</strong>
                <small>Auto-approve token-spending providers.</small>
              </span>
            </label>
            <label className={`orchestrate-approval-card ${approveCodingAgents ? "selected" : ""}`}>
              <input
                type="checkbox"
                checked={approveCodingAgents}
                onChange={(e) => setApproveCodingAgents(e.target.checked)}
              />
              <span>
                <strong>CLI agents</strong>
                <small>Auto-approve local coding-agent processes.</small>
              </span>
            </label>
          </div>
        </div>

        {(() => {
          const previewed = orchestrate.plan.data;
          if (!previewed) {
            return (
              <p className="muted" style={{ marginTop: 16, fontSize: 13 }}>
                Tip: <strong>Preview plan</strong> first to see exactly which steps RepoDesk will run on its own and which need your approval.
              </p>
            );
          }
          const hasPaid = planHasPaidStep(previewed);
          const hasCoding = planHasCodingAgentStep(previewed);
          const pausePaid = hasPaid && !approvePaid;
          const pauseCoding = hasCoding && !approveCodingAgents;
          const willPause = pausePaid || pauseCoding;
          const gatedCount = previewed.steps.filter(stepIsApprovalGated).length;
          const autoCount = previewed.steps.length - gatedCount;
          return (
            <div className={`notice orchestrate-run-notice ${willPause ? "warn" : "ok"}`}>
              <strong>What runs without asking</strong>
              <p>
                RepoDesk will run <strong>{autoCount}</strong> step{autoCount === 1 ? "" : "s"} automatically.
                {gatedCount > 0 && willPause &&
                  ` It will pause for your approval on ${gatedCount} paid/agent step${gatedCount === 1 ? "" : "s"} before spending money or launching a CLI agent.`}
                {gatedCount > 0 && !willPause &&
                  ` You've pre-approved the ${gatedCount} paid/agent step${gatedCount === 1 ? "" : "s"} — they'll run without pausing.`}
                {gatedCount === 0 && " No paid or CLI-agent steps in this plan."}
              </p>
            </div>
          );
        })()}

        <div className="button-row orchestrate-action-row">
          <button className="ghost-button" onClick={() => void handlePreview()} disabled={busy}>
            {orchestrate.plan.isPending ? "Building..." : "Preview plan"}
          </button>
          <button className="ghost-button" onClick={() => void handleRun(true)} disabled={busy} title="Preview single run without executing">
            Dry run
          </button>
          <button className="primary-button" onClick={() => void handleRun(false)} disabled={busy}>
            {orchestrate.run.isPending ? "Running..." : "Run single step"}
          </button>
          <button className="primary-button" onClick={() => void handleLoop(false)} disabled={busy}>
            {orchestrate.loop.isPending ? "Looping..." : "Run autonomous loop"}
          </button>
        </div>
        <p className="muted orchestrate-run-note">
          <strong>Run single step</strong>: one pass, then you review the result.
          {" "}<strong>Run autonomous loop</strong>: retries up to {Number(maxIterations) || 3} time{(Number(maxIterations) || 3) === 1 ? "" : "s"}, stopping at your cost ceiling or any guardrail.
        </p>
      </section>

      {error && (
        <section className="panel" style={{ borderColor: "#f85149" }}>
          <p className="eyebrow" style={{ color: "#f85149" }}>
            Error
          </p>
          <p className="muted" style={{ margin: 0 }}>
            {errorToMessage(error)}
          </p>
        </section>
      )}

      <ExecutorStatusPanel executors={orchestrate.executors} loading={orchestrate.executorsLoading} />

      <WorktreeRecoveryPanel
        worktrees={orchestrate.worktrees}
        loading={orchestrate.worktreesLoading}
        cleaningWorkspace={cleaningWorkspace}
        onCleanup={(worktree) => void handleCleanupWorktree(worktree)}
      />

      {orchestrate.plan.data && <PlanPanel plan={orchestrate.plan.data} />}

      {orchestrate.loop.data && <LoopPanel loop={orchestrate.loop.data} />}

      {currentRun && (
        <ReviewCenter
          run={currentRun}
          diffs={orchestrate.runDiffs.data ?? []}
          diffsLoading={orchestrate.runDiffs.isPending}
          proof={orchestrate.checkProof.data ?? null}
          proofLoading={orchestrate.checkProof.isPending}
          onRefreshProof={() => orchestrate.checkProof.mutate({ runId: currentRun.run_id, runChecks: false })}
          onRunChecks={() => void handleRunChecksForCurrentRun()}
          onReview={(action) => void handleReview(action)}
          reviewing={orchestrate.review.isPending}
          acceptingAndVerifying={acceptingAndVerifying}
          reviewResult={reviewResult?.run_id === currentRun.run_id ? reviewResult : null}
        />
      )}

      {currentRun && (
        <RunPanel
          run={currentRun}
          onOpenMemory={setActiveTab ? () => setActiveTab("memory") : undefined}
          reviewResult={reviewResult?.run_id === currentRun.run_id ? reviewResult : null}
        />
      )}

      <HistoryPanel
        runs={orchestrate.runs}
        activeRunId={currentRun?.run_id}
        onSelect={(runId) => void selectRun(runId)}
        busy={orchestrate.showRun.isPending}
      />

      <TimelinePanel events={orchestrate.timeline} />
    </div>
  );
}
