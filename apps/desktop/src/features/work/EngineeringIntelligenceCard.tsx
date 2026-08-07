import { useQuery } from "@tanstack/react-query";
import {
  workEngineeringIntelligence,
  type EngineeringIntelligence,
} from "../../shared/api/engineering";
import { useWorkspace } from "../../shared/hooks/useWorkspace";

const INTELLIGENCE_KEY = ["work", "engineering-intelligence"] as const;

function rate(value: number | null): string {
  return value == null ? "—" : `${Math.round(value * 100)}%`;
}

function Metric({ label, value, detail }: { label: string; value: string; detail: string }) {
  return (
    <div className="phase-brief-cell">
      <span>{label}</span>
      <strong>{value}</strong>
      <small className="muted">{detail}</small>
    </div>
  );
}

function summary(report: EngineeringIntelligence): string {
  if (report.event_count === 0) {
    return "No engineering evidence has been recorded for this task yet.";
  }

  if (report.completion.committed) {
    return "This task has commit evidence. Metrics below are replayed from its append-only engineering ledger.";
  }

  if (report.verification.failed > 0) {
    return "Verification has failing evidence. RepoDesk reports the facts without collapsing them into a quality score.";
  }

  if (report.changes.pending_review_changesets > 0) {
    return "A changeset is still awaiting review. The evidence view updates as the task progresses.";
  }

  return "Deterministic task evidence derived from the engineering event ledger.";
}

export function EngineeringIntelligenceCard() {
  const { hasTask } = useWorkspace();
  const intelligence = useQuery({
    queryKey: INTELLIGENCE_KEY,
    queryFn: workEngineeringIntelligence,
    enabled: hasTask,
    refetchInterval: 4000,
  });

  if (!hasTask) return null;

  if (intelligence.isLoading || !intelligence.data) {
    return (
      <div className="content-grid">
        <section className="panel wide-panel">
          <p className="eyebrow">Engineering Intelligence</p>
          <p className="muted">Loading task evidence…</p>
        </section>
      </div>
    );
  }

  if (intelligence.isError) {
    return (
      <div className="content-grid">
        <section className="panel wide-panel">
          <p className="eyebrow">Engineering Intelligence</p>
          <p className="notice danger">Could not load task intelligence: {String(intelligence.error)}</p>
        </section>
      </div>
    );
  }

  const report = intelligence.data;
  const totalAiTokens = report.ai_usage.input_tokens + report.ai_usage.output_tokens;
  const latestCommit = report.completion.latest_commit_sha?.slice(0, 12) ?? "not committed";

  return (
    <div className="content-grid">
      <section className="panel wide-panel" aria-label="Engineering Intelligence">
        <div className="phase-brief-head">
          <div>
            <p className="eyebrow">Engineering Intelligence</p>
            <h3>Evidence from this work item</h3>
          </div>
          <span className="pill accent">{report.event_count} ledger events</span>
        </div>

        <p className="muted">{summary(report)}</p>

        <div className="phase-brief-grid">
          <Metric
            label="Execution"
            value={`${report.execution.completed}/${report.execution.attempts}`}
            detail={`${report.execution.unique_workers} workers · ${report.execution.handoffs} handoffs · ${rate(
              report.rates.execution_completion_rate,
            )} completion`}
          />
          <Metric
            label="Context"
            value={report.context.latest_estimated_tokens?.toLocaleString() ?? "—"}
            detail={`${report.context.builds} builds · ${report.context.total_estimated_tokens.toLocaleString()} estimated tokens total`}
          />
          <Metric
            label="Changes"
            value={`${report.changes.accepted_changesets} accepted`}
            detail={`${report.changes.proposed_files} proposed files · ${report.changes.rejected_changesets} rejected · ${report.changes.pending_review_changesets} pending`}
          />
          <Metric
            label="Verification"
            value={`${report.verification.passed}/${report.verification.finished}`}
            detail={`${report.verification.commands_run} commands · ${report.verification.failed} failed · ${rate(
              report.rates.verification_pass_rate,
            )} pass rate`}
          />
          <Metric
            label="AI footprint"
            value={totalAiTokens.toLocaleString()}
            detail={`${report.ai_usage.input_tokens.toLocaleString()} in · ${report.ai_usage.output_tokens.toLocaleString()} out · ${report.ai_usage.cost_units.toFixed(3)} cost units`}
          />
          <Metric
            label="Completion"
            value={report.completion.committed ? "Committed" : "Open"}
            detail={`${report.completion.commits} commits · ${report.completion.committed_files} files · ${latestCommit}`}
          />
        </div>

        <p className="muted">
          Acceptance rate: {rate(report.rates.changeset_acceptance_rate)}. No composite productivity or AI score is calculated.
        </p>
      </section>
    </div>
  );
}
