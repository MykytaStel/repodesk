import { useQuery } from "@tanstack/react-query";
import {
  WORK_OBSERVABILITY_KEY,
  workObservabilitySnapshot,
  type AiUsageReport,
  type AiUsageSignal,
  type StrategyFeedbackReport,
  type StrategyProfileFeedback,
} from "../../shared/api/observability";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { errorToMessage } from "../../shared/utils/helpers";

function pct(value: number | null): string {
  return value == null ? "—" : `${Math.round(value * 100)}%`;
}

function number(value: number | null, digits = 1): string {
  return value == null ? "—" : value.toLocaleString(undefined, { maximumFractionDigits: digits });
}

function signalTone(signal: AiUsageSignal): string {
  return signal.severity === "warning" ? "warning" : "info";
}

function useWorkObservability() {
  const { hasTask } = useWorkspace();
  return useQuery({
    queryKey: WORK_OBSERVABILITY_KEY,
    queryFn: workObservabilitySnapshot,
    enabled: hasTask,
    staleTime: 2_000,
    refetchOnWindowFocus: true,
  });
}

export function WorkIntelligenceRailSummary() {
  const snapshot = useWorkObservability();
  const report = snapshot.data?.ai_usage_report ?? null;
  const feedback = snapshot.data?.strategy_feedback ?? null;

  if (snapshot.isLoading) {
    return <section className="work-ai-rail-summary muted">Reading AI usage…</section>;
  }
  if (!report) return null;

  const warnings = report.signals.filter((signal) => signal.severity === "warning").length;
  const repeated = report.context.latest_repeated_context_ratio;

  return (
    <section className={`work-ai-rail-summary${warnings > 0 ? " warning" : ""}`} aria-label="AI usage summary">
      <div className="work-ai-rail-head">
        <span>AI usage</span>
        <strong>{report.total_tokens.toLocaleString()} tok</strong>
      </div>
      <div className="work-ai-rail-grid">
        <span><strong>{report.orchestration.unique_coding_agents}</strong> agents</span>
        <span><strong>{report.orchestration.handoffs}</strong> handoffs</span>
        <span><strong>{repeated == null ? "—" : `${Math.round(repeated * 100)}%`}</strong> repeated</span>
        <span><strong>{warnings}</strong> warnings</span>
      </div>
      {feedback && feedback.strategy_runs > 0 ? (
        <div className="work-ai-rail-learning">
          <span>Strategy evidence</span>
          <strong>{feedback.settled_runs}/{feedback.strategy_runs} settled</strong>
        </div>
      ) : null}
    </section>
  );
}

function Metric({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <div className="work-ai-metric">
      <span>{label}</span>
      <strong>{value}</strong>
      {hint ? <small>{hint}</small> : null}
    </div>
  );
}

function SignalRow({ signal }: { signal: AiUsageSignal }) {
  return (
    <article className={`work-ai-signal ${signalTone(signal)}`}>
      <header>
        <span className={`pill ${signal.severity === "warning" ? "warn" : "neutral"}`}>{signal.severity}</span>
        <strong>{signal.title}</strong>
      </header>
      <p>{signal.detail}</p>
      <small>{signal.recommendation}</small>
    </article>
  );
}

const PROFILE_LABELS = {
  lean: "Lean",
  balanced: "Balanced",
  local_first: "Local-first",
  quality: "Quality",
} as const;

function StrategyProfileRow({ value }: { value: StrategyProfileFeedback }) {
  return (
    <div className={`strategy-feedback-row${value.adaptation_ready ? " adaptation-ready" : ""}`}>
      <div>
        <strong>{PROFILE_LABELS[value.profile]}</strong>
        <small>{value.adaptation_ready ? "Auto may use this history" : "Collecting evidence"}</small>
      </div>
      <span><strong>{value.settled_runs}</strong><small>settled</small></span>
      <span><strong>{pct(value.success_rate)}</strong><small>success</small></span>
      <span><strong>{number(value.average_actual_tokens, 0)}</strong><small>avg tokens</small></span>
      <span><strong>{pct(value.average_token_estimate_error_ratio)}</strong><small>estimate error</small></span>
    </div>
  );
}

function StrategyFeedbackSection({ feedback }: { feedback: StrategyFeedbackReport | null }) {
  if (!feedback || feedback.strategy_runs === 0) {
    return (
      <section className="work-ai-section strategy-feedback-section">
        <header><strong>Strategy feedback</strong><span>cold start</span></header>
        <div className="work-ai-empty">
          Auto is evidence-aware but has no settled strategy runs yet. Three settled outcomes for a profile are required before history can adapt future Auto decisions.
        </div>
      </section>
    );
  }

  return (
    <section className="work-ai-section strategy-feedback-section">
      <header>
        <strong>Strategy feedback</strong>
        <span>{feedback.settled_runs} settled · {feedback.pending_runs} pending</span>
      </header>
      <div className="strategy-feedback-table">
        {feedback.profiles.map((profile) => <StrategyProfileRow key={profile.profile} value={profile} />)}
      </div>
      <p className="strategy-feedback-note">
        Pending runs do not count as failures. Current execution/review/verification instability always overrides historical efficiency.
      </p>
    </section>
  );
}

function IntelligenceBody({ report, feedback }: { report: AiUsageReport; feedback: StrategyFeedbackReport | null }) {
  const context = report.context;
  const orchestration = report.orchestration;
  const outcomes = report.outcomes;
  const savedRatio = context.total_candidate_tokens > 0
    ? context.total_saved_tokens / context.total_candidate_tokens
    : null;

  return (
    <div className="work-ai-intelligence">
      <section className="work-ai-summary">
        <div>
          <span className="eyebrow">AI usage intelligence</span>
          <h3>{report.total_tokens.toLocaleString()} tokens observed</h3>
          <p>Explainable usage evidence only. RepoDesk does not compress this into an opaque productivity score.</p>
        </div>
        <div className="work-ai-cost">
          <span>Cost units</span>
          <strong>{report.cost_units.toFixed(4)}</strong>
        </div>
      </section>

      <section className="work-ai-section">
        <header><strong>Context efficiency</strong><span>{context.builds} build{context.builds === 1 ? "" : "s"}</span></header>
        <div className="work-ai-metrics">
          <Metric label="Latest packet" value={context.latest_included_tokens?.toLocaleString() ?? "—"} hint="included tokens" />
          <Metric label="Candidate" value={context.latest_candidate_tokens?.toLocaleString() ?? "—"} hint="before packing" />
          <Metric label="Saved" value={pct(savedRatio)} hint={`${context.total_saved_tokens.toLocaleString()} tokens compacted`} />
          <Metric label="Repeated" value={pct(context.latest_repeated_context_ratio)} hint={context.latest_repeated_tokens == null ? "no comparison" : `${context.latest_repeated_tokens.toLocaleString()} tokens`} />
        </div>
      </section>

      <section className="work-ai-section">
        <header><strong>Orchestration</strong><span>{orchestration.managed_executions} managed run{orchestration.managed_executions === 1 ? "" : "s"}</span></header>
        <div className="work-ai-metrics">
          <Metric label="Coding agents" value={orchestration.unique_coding_agents.toString()} />
          <Metric label="Workers" value={orchestration.unique_workers.toString()} />
          <Metric label="Handoffs" value={orchestration.handoffs.toString()} />
          <Metric label="Handoffs / run" value={number(orchestration.handoffs_per_managed_execution)} />
        </div>
      </section>

      <section className="work-ai-section">
        <header><strong>Outcome efficiency</strong><span>{outcomes.completed_executions} completed</span></header>
        <div className="work-ai-metrics">
          <Metric label="Execution completion" value={pct(outcomes.execution_completion_rate)} />
          <Metric label="Change acceptance" value={pct(outcomes.changeset_acceptance_rate)} />
          <Metric label="Verification pass" value={pct(outcomes.verification_pass_rate)} />
          <Metric label="Tokens / accepted file" value={number(outcomes.tokens_per_accepted_file, 0)} />
          <Metric label="Tokens / execution" value={number(outcomes.tokens_per_finished_execution, 0)} />
          <Metric label="Input / output" value={outcomes.input_output_ratio == null ? "—" : `${number(outcomes.input_output_ratio)}:1`} />
        </div>
      </section>

      <StrategyFeedbackSection feedback={feedback} />

      <section className="work-ai-section work-ai-signals">
        <header><strong>RepoDesk signals</strong><span>{report.signals.length}</span></header>
        {report.signals.length === 0 ? (
          <div className="work-ai-empty">No deterministic efficiency warning is supported by the current evidence.</div>
        ) : (
          report.signals.map((signal) => <SignalRow key={signal.code} signal={signal} />)
        )}
      </section>
    </div>
  );
}

export function WorkIntelligenceCard() {
  const snapshot = useWorkObservability();

  if (snapshot.isLoading) return <div className="focus-empty compact">Deriving AI usage intelligence…</div>;
  if (snapshot.isError) return <div className="notice danger">{errorToMessage(snapshot.error)}</div>;
  const report = snapshot.data?.ai_usage_report;
  if (!report) return <div className="focus-empty compact">No active Work Item intelligence is available.</div>;

  return <IntelligenceBody report={report} feedback={snapshot.data?.strategy_feedback ?? null} />;
}
