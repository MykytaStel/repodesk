import type { StrategyExecutionContract } from "../../shared/api/strategy";
import {
  ErrorState,
  EvidenceState,
  LoadingState,
  Metric,
  PanelHeader,
  StatusBadge,
} from "../../shared/ui/primitives";
import { packetPreparationSemantic } from "./workSemantic";

function shortFingerprint(value: string | null): string {
  if (!value) return "—";
  return value.length <= 18 ? value : `${value.slice(0, 10)}…${value.slice(-6)}`;
}

export function ExecutionPreviewCompact({
  preview,
  loading,
  error,
}: {
  preview: StrategyExecutionContract | null;
  loading: boolean;
  error: string | null;
}) {
  if (loading) return <LoadingState message="Preparing execution contract…" />;
  if (error) return <ErrorState title="Run preview unavailable" detail={error} />;
  if (!preview || preview.steps.length === 0) return null;

  const lead = preview.steps.find((step) => step.allow_write) ?? preview.steps[preview.steps.length - 1];
  const context = preview.context;
  const preparation = packetPreparationSemantic(context.prepared);
  const budgetCopy = context.prepared
    ? `${context.context_tokens.toLocaleString()}${context.token_budget ? ` / ${context.token_budget.toLocaleString()}` : ""}`
    : "build on launch";

  return (
    <section className="exec-packet" aria-label="Execution packet preview">
      <PanelHeader
        eyebrow="Execution packet"
        title={`${lead?.executor_label ?? "Worker"} · ${lead?.model ?? "provider default"}`}
        description="The approved packet below is the boundary RepoDesk will hand to this run."
        trailing={<StatusBadge label={preparation.label} tone={preparation.tone} />}
      />

      <div className="exec-packet-grid">
        <Metric
          label="Context"
          value={budgetCopy}
          detail={`tokens · ${context.included_sources} in / ${context.excluded_sources} out`}
          tone={context.prepared ? "positive" : "attention"}
        />
        <Metric
          label="Workspace"
          value={preview.isolated_workspace ? "Isolated" : "Active checkout"}
          detail={preview.expected_writes ? "writes expected" : "read-only route"}
          tone={preview.isolated_workspace ? "positive" : "neutral"}
        />
        <Metric
          label="Run estimate"
          value={`${preview.total_estimated_tokens.toLocaleString()} tokens`}
          detail="context + planned outputs"
        />
        <Metric
          label="Cost ceiling view"
          value={`${preview.total_estimated_cost_units.toFixed(2)} ${preview.currency_label}`}
          detail={preview.requires_paid_approval ? "paid approval required" : "no paid approval"}
          tone={preview.requires_paid_approval ? "attention" : "neutral"}
        />
      </div>

      <div className="exec-packet-boundary">
        <span>
          <small>Packet fingerprint</small>
          <code title={context.context_fingerprint ?? undefined}>{shortFingerprint(context.context_fingerprint)}</code>
        </span>
        <span>
          <small>Sources</small>
          <strong>{context.prepared ? `${context.included_sources} selected` : "not prepared"}</strong>
        </span>
        <span>
          <small>Write scope</small>
          <strong>{preview.expected_writes ? (preview.isolated_workspace ? "run worktree only" : "workspace") : "none"}</strong>
        </span>
      </div>

      {context.warning ? (
        <EvidenceState label="Context" state="Attention" tone="attention" detail={context.warning} />
      ) : null}

      <details className="exec-packet-routing">
        <summary>{preview.steps.length} routed step{preview.steps.length === 1 ? "" : "s"} · inspect routing</summary>
        <div className="exec-packet-step-list">
          {preview.steps.map((step, index) => (
            <div key={step.step_id}>
              <span>{String(index + 1).padStart(2, "0")}</span>
              <div>
                <strong>{step.title}</strong>
                <small>{step.executor_label} · {step.model}</small>
              </div>
              <code>{step.estimated_input_tokens.toLocaleString()} → {step.estimated_output_tokens.toLocaleString()}</code>
              <span>{step.allow_write ? "write" : "read"}{step.paid ? " · paid" : ""}</span>
            </div>
          ))}
        </div>
      </details>
    </section>
  );
}
