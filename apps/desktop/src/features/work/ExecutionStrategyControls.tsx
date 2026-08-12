import type {
  AiStrategyMode,
  StrategyExecutionPreview,
} from "../../shared/api/strategy";

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
      <header className="ai-strategy-heading">
        <div>
          <span className="eyebrow">AI strategy</span>
          <strong>
            {loading
              ? "Evaluating execution shape…"
              : strategy
                ? `${mode === "auto" ? "Auto → " : ""}${PROFILE_LABELS[strategy.profile]}`
                : "Strategy unavailable"}
          </strong>
          <small>RepoDesk may reduce AI orchestration, never the human review or verification gates.</small>
        </div>
        {strategy ? (
          <span className={`ai-strategy-badge profile-${strategy.profile}`}>
            {SHAPE_LABELS[strategy.plan_shape]}
          </span>
        ) : null}
      </header>

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
          <div className="ai-strategy-comparison">
            <div>
              <span>AI calls</span>
              <strong>{comparison.baseline_steps} → {comparison.planned_steps}</strong>
              <small>baseline → selected</small>
            </div>
            <div>
              <span>Token estimate</span>
              <strong>
                {comparison.estimated_saved_tokens > 0
                  ? `−${comparison.estimated_saved_tokens.toLocaleString()}`
                  : "no reduction"}
              </strong>
              <small>{comparison.planned_estimated_tokens.toLocaleString()} planned</small>
            </div>
            <div>
              <span>Cost delta</span>
              <strong>{formatDelta(comparison.estimated_cost_delta_units, currency)}</strong>
              <small>vs balanced baseline</small>
            </div>
            <div>
              <span>Context</span>
              <strong>{strategy.reuse_prepared_context ? "reuse prepared" : "build required"}</strong>
              <small>{strategy.economy_mode} routing</small>
            </div>
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
