import { useState } from "react";
import { useOrchestrate } from "./useOrchestrate";
import {
  planHasPaidStep,
  type OrchestrationPlan,
  type OrchestrationRun,
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
    <section className="panel">
      <p className="eyebrow">Plan</p>
      <h2 style={{ marginTop: 4 }}>{plan.goal}</h2>
      <div className="table-list" style={{ marginTop: 8 }}>
        {plan.steps.map((step) => (
          <div className="table-row flex-col items-start gap-sm" key={step.id} style={{ paddingBottom: 12 }}>
            <div className="w-full flex justify-between items-center">
              <strong>
                {step.id} — {step.title}
              </strong>
              {step.allow_write ? (
                <span style={{ color: "#d29922", fontSize: 12 }}>writes</span>
              ) : (
                <span className="muted" style={{ fontSize: 12 }}>
                  read-only
                </span>
              )}
            </div>
            <div className="row-meta">
              <span>
                {step.agent} / {step.model ?? "default model"}
              </span>
              <span>thinking: {step.thinking}</span>
              <span>
                depends on: {step.depends_on.length ? step.depends_on.join(", ") : "none"}
              </span>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function RunPanel({
  run,
  onOpenMemory,
}: {
  run: OrchestrationRun;
  onOpenMemory?: () => void;
}) {
  const captured = run.results.reduce((sum, r) => sum + r.captured_proposals, 0);
  return (
    <section className="panel">
      <p className="eyebrow">
        Run {run.run_id} — {run.status}
        {run.dry_run ? " (dry run)" : ""}
      </p>
      <div className="content-grid" style={{ gridTemplateColumns: "repeat(3, 1fr)", marginBottom: 12 }}>
        <MetricCard label="Input tokens" value={formatNumber(run.total_input_tokens)} detail="across sub-agents" />
        <MetricCard label="Output tokens" value={formatNumber(run.total_output_tokens)} detail="across sub-agents" />
        <MetricCard label="Cost" value={formatCost(run.total_cost_units, "units")} detail={run.dry_run ? "projected" : "actual"} />
      </div>
      <div className="table-list">
        {run.results.map((result: SubAgentResult) => (
          <div className="table-row flex-col items-start gap-sm" key={result.task_id} style={{ paddingBottom: 12 }}>
            <div className="w-full flex justify-between items-center">
              <strong>
                {result.task_id} — {result.provider}
                {result.model ? ` / ${result.model}` : ""}
              </strong>
              <StatusBadge status={result.status} />
            </div>
            <div className="row-meta">
              <span>in {formatNumber(result.input_tokens)}</span>
              <span>out {formatNumber(result.output_tokens)}</span>
              <span>cost {formatCost(result.cost_units, "units")}</span>
              <span>captured {result.captured_proposals}</span>
            </div>
            {result.notes.map((note, i) => (
              <p className="muted" key={i} style={{ fontSize: 12, margin: 0 }}>
                {note}
              </p>
            ))}
          </div>
        ))}
      </div>
      {!run.dry_run && captured > 0 && (
        <div className="button-row compact-buttons" style={{ marginTop: 12 }}>
          <button className="tiny-button" onClick={onOpenMemory} disabled={!onOpenMemory}>
            Review {captured} captured memory proposal{captured === 1 ? "" : "s"}
          </button>
        </div>
      )}
    </section>
  );
}

function fmtTime(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString();
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
      <p className="eyebrow">Task activity</p>
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
  const [goal, setGoal] = useState("");
  const [maxCost, setMaxCost] = useState("");
  const [selectedRun, setSelectedRun] = useState<OrchestrationRun | null>(null);

  const busy = orchestrate.plan.isPending || orchestrate.run.isPending;
  // A run just executed wins; otherwise a history selection; otherwise the latest run.
  const currentRun = orchestrate.run.data ?? selectedRun ?? orchestrate.status;
  const error = orchestrate.run.error ?? orchestrate.plan.error ?? orchestrate.showRun.error;

  async function selectRun(runId: string) {
    const detail = await orchestrate.showRun.mutateAsync(runId);
    if (detail) setSelectedRun(detail);
  }

  async function handlePreview() {
    await orchestrate.plan.mutateAsync(goal);
  }

  async function handleRun(dryRun: boolean) {
    const built = await orchestrate.plan.mutateAsync(goal);
    if (!dryRun && planHasPaidStep(built)) {
      const ok = window.confirm(
        "This plan includes paid provider steps that will spend tokens. Run for real?",
      );
      if (!ok) return;
    }
    const parsedCost = maxCost.trim() ? Number(maxCost) : null;
    await orchestrate.run.mutateAsync({
      goal,
      dryRun,
      maxCost: Number.isFinite(parsedCost as number) ? parsedCost : null,
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

        <textarea
          value={goal}
          onChange={(e) => setGoal(e.target.value)}
          placeholder="Optional goal override (defaults to the active task title)"
          rows={2}
          style={{ width: "100%", marginTop: 12 }}
        />

        <div className="button-row compact-buttons" style={{ marginTop: 12, alignItems: "center" }}>
          <button className="tiny-button" onClick={() => void handlePreview()} disabled={busy}>
            {orchestrate.plan.isPending ? "Building..." : "Preview plan"}
          </button>
          <button className="tiny-button" onClick={() => void handleRun(true)} disabled={busy}>
            Dry run
          </button>
          <button className="tiny-button" onClick={() => void handleRun(false)} disabled={busy}>
            {orchestrate.run.isPending ? "Running..." : "Run"}
          </button>
          <label className="muted" style={{ fontSize: 12, display: "inline-flex", gap: 6, alignItems: "center" }}>
            max cost
            <input
              value={maxCost}
              onChange={(e) => setMaxCost(e.target.value)}
              placeholder="units"
              inputMode="decimal"
              style={{ width: 80 }}
            />
          </label>
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

      {orchestrate.plan.data && <PlanPanel plan={orchestrate.plan.data} />}

      {currentRun && (
        <RunPanel run={currentRun} onOpenMemory={setActiveTab ? () => setActiveTab("memory") : undefined} />
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
