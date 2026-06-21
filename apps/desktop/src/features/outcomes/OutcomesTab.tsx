import { useOutcomes } from "./useOutcomes";
import type { OutcomeRecord, ProviderStat, Verdict } from "../../shared/api/orchestrate";
import { EmptyState, formatCost, formatNumber } from "../../shared/ui/SharedComponents";
import { CheckIcon } from "../../app/NavIcons";

const VERDICT_TONE: Record<Verdict, "ok" | "danger" | "neutral"> = {
  good: "ok",
  bad: "danger",
  neutral: "neutral",
};

function fmtTime(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString();
}

function successLabel(stat: ProviderStat): string {
  if (stat.success_rate === null) return "—";
  return `${Math.round(stat.success_rate * 100)}%`;
}

function StatsPanel({ stats }: { stats: ProviderStat[] }) {
  return (
    <section className="panel wide-panel">
      <div className="panel-title-row compact">
        <p className="eyebrow" style={{ margin: 0 }}>What the brain learned</p>
        <span className="pill">{stats.length} pair{stats.length === 1 ? "" : "s"}</span>
      </div>
      {stats.length === 0 ? (
        <EmptyState
          message="No learning signal yet."
          hint="Run the orchestrator (or the autonomous loop) — each step is recorded and the router adapts."
        />
      ) : (
        <div className="table-list">
          {stats.map((stat) => {
            const tone = stat.success_rate === null ? "neutral" : stat.success_rate >= 0.5 ? "ok" : "danger";
            return (
              <div className="table-row flex-col items-start gap-sm" key={`${stat.task_kind}-${stat.provider}`}>
                <div className="w-full flex justify-between items-center">
                  <strong>
                    {stat.task_kind} → {stat.provider}
                  </strong>
                  <span className={`pill ${tone}`}>{successLabel(stat)} success</span>
                </div>
                <div className="row-meta">
                  <span className="ok">{stat.good} good</span>
                  <span className="danger">{stat.bad} bad</span>
                  <span>{stat.neutral} neutral</span>
                  <span>avg cost {formatCost(stat.avg_cost_units, "units")}</span>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}

function OutcomeRow({
  row,
  onConfirm,
  busy,
}: {
  row: OutcomeRecord;
  onConfirm: (id: number, verdict: Verdict) => void;
  busy: boolean;
}) {
  return (
    <div className="table-row flex-col items-start gap-sm">
      <div className="w-full flex justify-between items-center">
        <strong>
          {row.task_kind} → {row.provider}
          {row.model ? ` / ${row.model}` : ""}
        </strong>
        <span className={`pill ${VERDICT_TONE[row.verdict]}`}>
          {row.verdict}
          {row.confirmed && (
            <span className="pill-inline-icon" aria-label="confirmed">
              <CheckIcon />
            </span>
          )}
        </span>
      </div>
      <div className="row-meta">
        <span>{row.status}</span>
        <span>cost {formatCost(row.cost_units, "units")}</span>
        <span>
          {formatNumber(row.input_tokens)} in / {formatNumber(row.output_tokens)} out
        </span>
        <span className="muted">{row.verdict_source}</span>
        <span className="muted">{fmtTime(row.created_at)}</span>
      </div>
      <div className="button-row compact-buttons" style={{ marginTop: 0 }}>
        <button className="tiny-button" disabled={busy} onClick={() => onConfirm(row.id, "good")}>
          Mark good
        </button>
        <button className="tiny-button" disabled={busy} onClick={() => onConfirm(row.id, "bad")}>
          Mark bad
        </button>
      </div>
    </div>
  );
}

export function OutcomesTab() {
  const outcomes = useOutcomes();

  if (!outcomes.ready) {
    return (
      <div className="content-grid">
        <section className="hero-panel wide-panel">
          <p className="eyebrow">Learning</p>
          <h1>No active project + task</h1>
          <p className="lead">
            Connect a project and create a task. As the orchestrator runs, each step's outcome is
            recorded here and feeds the adaptive router.
          </p>
        </section>
      </div>
    );
  }

  return (
    <div className="content-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Learning</p>
        <h1>Outcome ledger</h1>
        <p className="lead">
          Every orchestrator step is recorded with a verdict. The adaptive router reads these to
          prefer what has worked on this project — confirmed verdicts count double. Confirm or
          override a verdict to steer the brain.
        </p>
      </section>

      <StatsPanel stats={outcomes.stats} />

      <section className="panel wide-panel">
        <div className="panel-title-row compact">
          <p className="eyebrow" style={{ margin: 0 }}>Recent outcomes</p>
          <span className="pill">{outcomes.list.length}</span>
        </div>
        {outcomes.list.length === 0 ? (
          <EmptyState message="No recorded outcomes yet." hint="Run the orchestrator to start the learning loop." />
        ) : (
          <div className="table-list">
            {outcomes.list.map((row) => (
              <OutcomeRow
                key={row.id}
                row={row}
                busy={outcomes.confirm.isPending}
                onConfirm={(id, verdict) => outcomes.confirm.mutate({ id, verdict })}
              />
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
