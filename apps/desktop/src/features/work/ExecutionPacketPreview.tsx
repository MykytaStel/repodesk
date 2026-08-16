import type { ExecutionPreview } from "../../shared/api/orchestrate";
import {
  ErrorState,
  EvidenceState,
  InspectorSection,
  LoadingState,
  Metric,
  PanelHeader,
  StatusBadge,
} from "../../shared/ui/primitives";
import { preparedContextSemanticState } from "./workSemantic";

export type ExecutionPacketPreviewData = ExecutionPreview & {
  context: {
    prepared: boolean;
    context_tokens: number;
    candidate_tokens: number;
    token_budget: number | null;
    included_sources: number;
    excluded_sources: number;
    context_fingerprint: string | null;
    generated_at: string | null;
    warning: string | null;
  };
};

function shortFingerprint(value: string | null): string {
  if (!value) return "—";
  return value.length <= 18 ? value : `${value.slice(0, 10)}…${value.slice(-6)}`;
}

export function ExecutionPacketPreview({
  preview,
  loading,
  error,
}: {
  preview: ExecutionPacketPreviewData | null;
  loading: boolean;
  error: string | null;
}) {
  if (loading) {
    return <LoadingState message="Preparing execution contract…" scope="inline" />;
  }
  if (error) {
    return (
      <ErrorState
        title="Execution packet unavailable"
        detail={error}
        scope="inline"
      />
    );
  }
  if (!preview || preview.steps.length === 0) return null;

  const lead = preview.steps.find((step) => step.allow_write) ?? preview.steps[preview.steps.length - 1];
  const context = preview.context;
  const contextSemantic = preparedContextSemanticState(context.prepared);
  const budgetCopy = context.prepared
    ? `${context.context_tokens.toLocaleString()}${context.token_budget ? ` / ${context.token_budget.toLocaleString()}` : ""}`
    : "Build on launch";
  const workspaceTone = preview.isolated_workspace && preview.expected_writes ? "positive" : "neutral";

  return (
    <section className="exec-packet" aria-label="Execution packet preview">
      <PanelHeader
        eyebrow="Execution packet"
        title={`${lead?.executor_label ?? "Worker"} · ${lead?.model ?? "provider default"}`}
        description="The approved packet below is the boundary RepoDesk will hand to this run."
        trailing={(
          <StatusBadge
            label={contextSemantic.label}
            tone={contextSemantic.tone}
            role="status"
            ariaLabel={`Execution context: ${contextSemantic.label}`}
          />
        )}
      />

      <div className="exec-packet-grid">
        <EvidenceState
          label="Context"
          state={contextSemantic.label}
          tone={contextSemantic.tone}
          detail={`${budgetCopy} tokens · ${context.included_sources} in / ${context.excluded_sources} out`}
        />
        <EvidenceState
          label="Workspace"
          state={preview.isolated_workspace ? "Isolated" : "Active checkout"}
          tone={workspaceTone}
          detail={preview.expected_writes ? "writes expected" : "read-only route"}
        />
        <Metric
          label="Run estimate"
          value={`${preview.total_estimated_tokens.toLocaleString()} tokens`}
          detail="context + planned outputs"
          tone="info"
        />
        <Metric
          label="Cost ceiling view"
          value={`${preview.total_estimated_cost_units.toFixed(2)} ${preview.currency_label}`}
          detail={preview.requires_paid_approval ? "paid approval required" : "no paid approval"}
          tone={preview.requires_paid_approval ? "attention" : "neutral"}
        />
      </div>

      <InspectorSection title="Packet boundary">
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
      </InspectorSection>

      {context.warning ? (
        <EvidenceState
          label="Execution context"
          state="Warning"
          tone="attention"
          detail={context.warning}
        />
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
