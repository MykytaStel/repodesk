import type {
  AiStrategyMode,
  StrategyExecutionPreview,
} from "../../shared/api/strategy";
import { Metric, PanelHeader, StatusBadge } from "../../shared/ui/primitives";

const MODES: Array<{ id: AiStrategyMode; label: string; hint: string }> = [
  { id: "auto", label: "Auto", hint: "Use RepoDesk evidence" },
  { id: "lean", label: "Lean", hint: "Minimize AI calls" },
  { id: "balanced", label: "Balanced", hint: "Default 3-step flow" },
  { id: "local_first", label: "Local-first", hint: "Prefer local reasoning" },
  { id: "quality", label: "Quality", hint: "Keep full review depth" },
];

const PROFILE_LABELS = {
  lean: "Lean",
  balanced: "Balanced",
  local_first: "Local-first",
  quality: "Quality",
} as const;

const SHAPE_LABELS = {
  single_writer: "1 writer",
  writer_with_review: "writer + AI review",
  analyze_writer_review: "analyze + writer + AI review",
} as const;

function formatDelta(value: number, currency: string): string {
  if (Math.abs(value) < 0.005) return `≈ same ${currency}`;
  const sign = value > 0 ? "+" : "−";
  return `${sign}${Math.abs(value).toFixed(2)} ${currency}`;
}

function shortFingerprint(value: string): string {
  return value.length > 18 ? `${value.slice(0, 12)}…${value.slice(-5)}` : value;
}

export function ExecutionStrategyControls({
  mode,
  preview,
  loading,
  onModeChange,
}: {
  mode: AiStrategyMode;
  preview: StrategyExecutionPreview | null;
  loading: boolean;
  onModeChange: (mode: AiStrategyMode) => void;
}) {
  const strategy = preview?.strategy ?? null;
  const comparison = preview?.comparison ?? null;
  const currency = preview?.execution.currency_label ?? "cost";

  return (
    <section className="ai-strategy-card" aria-label="AI execution strategy">
      <PanelHeader
        eyebrow="AI strategy"
        title={loading
          ? "Evaluating execution shape…"
          : strategy
            ? `${mode === "auto" ? "Auto → " : ""}${PROFILE_LABELS[strategy.profile]}`
            : "Strategy unavailable"}
        description="RepoDesk may reduce AI orchestration, never the human review or verification gates."
        trailing={strategy ? (
          <StatusBadge
            label={SHAPE_LABELS[strategy.plan_shape]}
            tone="info"
            className={`ai-strategy-badge profile-${strategy.profile}`}
          />
        ) : null}
      />

      <div className="ai-strategy-modes" role="radiogroup" aria-label="Execution strategy mode">
        {MODES.map((item) => (
          <button
            type="button"
            role="radio"
            aria-checked={mode === item.id}
            className={mode === item.id ? "selected" : ""}
            key={item.id}
            onClick={() => onModeChange(item.id)}
          >
            <strong>{item.label}</strong>
            <small>{item.hint}</small>
          </button>
        ))}
      </div>

      {strategy && comparison && preview ? (
        <>
          {strategy.feedback_influenced && strategy.feedback_detail ? (
            <div className="ai-strategy-feedback-influence" role="status">
              <span>Historical outcomes changed Auto</span>
              <strong>{strategy.feedback_detail}</strong>
            </div>
          ) : null}

          <div className="ai-strategy-comparison">
            <Metric
              label="AI calls"
              value={`${comparison.baseline_steps} → ${comparison.planned_steps}`}
              detail="baseline → selected"
            />
            <Metric
              label="Token estimate"
              value={comparison.estimated_saved_tokens > 0
                ? `−${comparison.estimated_saved_tokens.toLocaleString()}`
                : "no reduction"}
              detail={`${comparison.planned_estimated_tokens.toLocaleString()} planned`}
              tone={comparison.estimated_saved_tokens > 0 ? "positive" : "neutral"}
            />
            <Metric
              label="Cost delta"
              value={formatDelta(comparison.estimated_cost_delta_units, currency)}
              detail="vs balanced baseline"
              tone={comparison.estimated_cost_delta_units > 0 ? "attention" : "neutral"}
            />
            <Metric
              label="Context"
              value={strategy.reuse_prepared_context ? "reuse prepared" : "build required"}
              detail={`${strategy.economy_mode} routing`}
              tone={strategy.reuse_prepared_context ? "positive" : "attention"}
            />
          </div>

          <div className="ai-strategy-plan-lock">
            <span>Plan lock</span>
            <code title={preview.plan_fingerprint}>{shortFingerprint(preview.plan_fingerprint)}</code>
            <small>Launch stops if routing, strategy, or prepared context changes after this preview.</small>
          </div>

          <details className="ai-strategy-reasons" open={mode === "auto"}>
            <summary>Why RepoDesk chose this strategy</summary>
            <div>
              {strategy.reasons.slice(0, 6).map((reason) => (
                <p key={`${reason.code}:${reason.detail}`}>
                  <span>{reason.code.replace(/_/g, " ")}</span>
                  <small>{reason.detail}</small>
                </p>
              ))}
            </div>
          </details>
        </>
      ) : null}
    </section>
  );
}
