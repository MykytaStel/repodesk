import { useState } from "react";
import { useOrchestrate } from "./useOrchestrate";
import { useModels } from "../models/useModels";
import {
  planHasCodingAgentStep,
  planHasPaidStep,
  type ExecutorAvailability,
  type LoopRun,
  type LoopStatus,
  type OrchestrationPlan,
  type OrchestrationRun,
  type ReviewAction,
  type RunReview,
  type RunSummary,
  type SubAgentResult,
  type SubAgentStatus,
  type TaskEvent,
} from "../../shared/api/orchestrate";
import { MetricCard, errorToMessage, formatCost, formatNumber, EmptyState } from "../../shared/ui/SharedComponents";
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
    <section className="panel wide-panel">
      <p className="eyebrow">Plan</p>
      <h2 style={{ marginTop: 4 }}>{plan.goal}</h2>
      <div className="timeline" style={{ marginTop: 16 }}>
        {plan.steps.map((step, index) => {
          const executor = step.executor_id ?? step.agent;
          const provider = step.provider_id ?? step.provider;
          return (
            <div key={step.id} className="timeline-step neutral">
              <span>{index + 1}</span>
              <strong>
                {step.id} — {step.title}
              </strong>
              <small>{executor}{provider && provider !== executor ? ` → ${provider}` : ""} / {step.model ?? "default model"}</small>
              <p>
                {step.allow_write ? <strong style={{ color: "#d29922" }}>Writes permitted</strong> : "Read-only"}. 
                {step.depends_on.length > 0 && ` Depends on: ${step.depends_on.join(", ")}.`}
              </p>
            </div>
          );
        })}
      </div>
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
                <span>{executor.authenticated === true ? "authenticated" : "auth unknown"}</span>
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

function RunPanel({
  run,
  onOpenMemory,
  onReview,
  reviewing,
  reviewResult,
}: {
  run: OrchestrationRun;
  onOpenMemory?: () => void;
  onReview?: (action: ReviewAction) => void;
  reviewing?: boolean;
  reviewResult?: RunReview | null;
}) {
  const captured = run.results.reduce((sum, r) => sum + r.captured_proposals, 0);
  const changedCount = run.results.reduce((sum, r) => sum + (r.changed_files?.length ?? 0), 0);
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
          {changedCount > 0 && onReview && (
            <>
              <button
                className="tiny-button"
                onClick={() => onReview("accept")}
                disabled={reviewing}
                title="Stage the changed files so you can commit them"
              >
                Accept {changedCount} change{changedCount === 1 ? "" : "s"}
              </button>
              <button
                className="tiny-button"
                onClick={() => onReview("reject")}
                disabled={reviewing}
                title="Discard the agent's changes (restore tracked, remove new files)"
              >
                Reject changes
              </button>
            </>
          )}
        </div>
      )}
      {reviewResult && (
        <p className="muted" style={{ fontSize: 12, marginTop: 8 }}>
          {reviewResult.action === "accept" ? "Staged" : "Discarded"}:{" "}
          {reviewResult.processed.map((f) => `${f.path} (${f.outcome})`).join(", ") || "nothing"}
          {reviewResult.warnings.length > 0 ? ` — ${reviewResult.warnings.join("; ")}` : ""}
        </p>
      )}
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
  const [useIsolatedWorktree, setUseIsolatedWorktree] = useState(false);
  const [targetModelCombo, setTargetModelCombo] = useState<string>("auto");
  const [selectedRun, setSelectedRun] = useState<OrchestrationRun | null>(null);
  const [reviewResult, setReviewResult] = useState<RunReview | null>(null);

  const busy = orchestrate.plan.isPending || orchestrate.run.isPending || orchestrate.loop.isPending;
  // A run just executed wins; otherwise a history selection; otherwise the latest run.
  const currentRun = orchestrate.run.data ?? selectedRun ?? orchestrate.status;
  const error =
    orchestrate.run.error ?? orchestrate.plan.error ?? orchestrate.showRun.error ?? orchestrate.loop.error;

  async function handleReview(action: ReviewAction) {
    if (!currentRun) return;
    try {
      const result = await orchestrate.review.mutateAsync({ runId: currentRun.run_id, action });
      setReviewResult(result);
    } catch {
      // surfaced via orchestrate.review.error / the Error panel
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
      approveCodingAgents,
      useIsolatedWorktree,
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

        <div className="settings-grid" style={{ marginTop: 16, background: "var(--neutral-soft)", padding: 16, borderRadius: 8 }}>
          <label style={{ marginBottom: 16, display: "block" }}>
            Goal override
            <textarea
              value={goal}
              onChange={(e) => setGoal(e.target.value)}
              placeholder="Leave empty to default to the active task title"
              rows={2}
              style={{ width: "100%", marginTop: 8 }}
            />
          </label>
          
          <div className="form-grid" style={{ gridTemplateColumns: "1fr 1fr", gap: 16, display: "grid" }}>
            <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
              Target Model
              <select
                value={targetModelCombo}
                onChange={(e) => setTargetModelCombo(e.target.value)}
                style={{ width: "100%" }}
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
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
              <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                Max Cost (Units)
                <input
                  value={maxCost}
                  onChange={(e) => setMaxCost(e.target.value)}
                  placeholder="No limit"
                  inputMode="decimal"
                  style={{ width: "100%" }}
                />
              </label>
              <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                Max Loop Attempts
                <input
                  value={maxIterations}
                  onChange={(e) => setMaxIterations(e.target.value)}
                  inputMode="numeric"
                  style={{ width: "100%" }}
                />
              </label>
            </div>
          </div>
          
          <div style={{ display: "flex", gap: 24, marginTop: 16 }}>
            <label className="toggle-row" style={{ display: "flex", flexDirection: "row", alignItems: "center", gap: 8, cursor: "pointer" }}>
              <input type="checkbox" checked={approvePaid} onChange={(e) => setApprovePaid(e.target.checked)} style={{ margin: 0, width: "auto" }} />
              <span style={{ fontSize: 14 }}>Auto-approve paid steps</span>
            </label>
            <label className="toggle-row" style={{ display: "flex", flexDirection: "row", alignItems: "center", gap: 8, cursor: "pointer" }}>
              <input
                type="checkbox"
                checked={approveCodingAgents}
                onChange={(e) => setApproveCodingAgents(e.target.checked)}
                style={{ margin: 0, width: "auto" }}
              />
              <span style={{ fontSize: 14 }}>Auto-approve CLI agents</span>
            </label>
            <label className="toggle-row" style={{ display: "flex", flexDirection: "row", alignItems: "center", gap: 8, cursor: "pointer" }}>
              <input
                type="checkbox"
                checked={useIsolatedWorktree}
                onChange={(e) => setUseIsolatedWorktree(e.target.checked)}
                style={{ margin: 0, width: "auto" }}
              />
              <span style={{ fontSize: 14 }}>Isolated worktree</span>
            </label>
          </div>
        </div>

        <div className="button-row" style={{ marginTop: 16, gap: 12 }}>
          <button className="ghost-button" onClick={() => void handlePreview()} disabled={busy}>
            {orchestrate.plan.isPending ? "Building..." : "Preview plan"}
          </button>
          <button className="ghost-button" onClick={() => void handleRun(true)} disabled={busy} title="Preview single run without executing">
            Dry run
          </button>
          <button className="primary-button" onClick={() => void handleRun(false)} disabled={busy}>
            {orchestrate.run.isPending ? "Running..." : "Run single step"}
          </button>
          <button className="primary-button" style={{ background: "var(--accent)" }} onClick={() => void handleLoop(false)} disabled={busy}>
            {orchestrate.loop.isPending ? "Looping..." : "Run autonomous loop"}
          </button>
        </div>
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

      {orchestrate.plan.data && <PlanPanel plan={orchestrate.plan.data} />}

      {orchestrate.loop.data && <LoopPanel loop={orchestrate.loop.data} />}

      {currentRun && (
        <RunPanel
          run={currentRun}
          onOpenMemory={setActiveTab ? () => setActiveTab("memory") : undefined}
          onReview={(action) => void handleReview(action)}
          reviewing={orchestrate.review.isPending}
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
