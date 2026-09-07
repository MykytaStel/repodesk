import type { RecordDecisionInput, VerificationAdvisorSnapshot, VerificationDecisionKind } from "../../shared/api/verification";

type VerificationAdvisorCardProps = {
  snapshot: VerificationAdvisorSnapshot | null;
  isLoading: boolean;
  error: string | null;
  onRecord: (input: RecordDecisionInput) => void;
  onInspect: () => void;
};

function formatDuration(milliseconds: number | null): string {
  if (milliseconds == null) return "time unknown";
  const minutes = Math.max(1, Math.round(milliseconds / 60_000));
  return `~${minutes} min`;
}

function decisionAction(decision: VerificationDecisionKind): string {
  switch (decision) {
    case "run_now": return "Run required checks";
    case "run_targeted": return "Run targeted checks";
    case "defer_with_debt": return "Defer verification";
    case "ask_for_approval": return "Ask for approval";
    case "pause_and_review": return "Pause and review";
    case "stop_with_partial_result": return "Stop with partial result";
  }
}

function decisionInput(snapshot: VerificationAdvisorSnapshot, decision: VerificationDecisionKind): RecordDecisionInput {
  const recommendation = snapshot.recommendation;
  return {
    decision_id: null,
    project: snapshot.input.project,
    work_item_id: snapshot.input.work_item_id,
    execution_id: null,
    tree_identity: snapshot.input.tree_identity,
    changeset_identity: null,
    decision_kind: decision,
    policy_version: recommendation.policy_version,
    observed_facts: {
      changed_file_count: snapshot.input.changed_files.length,
      prior_attempts: snapshot.input.prior_attempts,
      prior_failures: snapshot.input.prior_failures,
      tests_observed: null,
    },
    evidence_refs: [],
    selected_actions: recommendation.selected_check_ids,
    skipped_actions: recommendation.deferred_checks.map((check) => check.check_id),
    estimated_cost_units: recommendation.estimated_cost_units,
    estimated_wall_clock_ms: recommendation.estimated_wall_clock_ms,
    risk_label: recommendation.risk_label,
    uncertainty_label: recommendation.uncertainty_label,
    verification_debt: recommendation.deferred_checks,
    human_override: decision === "ask_for_approval" ? "User requested approval after repeated failures." : null,
  };
}

export function VerificationAdvisorCard({ snapshot, isLoading, error, onRecord, onInspect }: VerificationAdvisorCardProps) {
  if (isLoading) {
    return <section className="work-control-card verification-advisor-card" aria-label="Verification Advisor"><span className="eyebrow">Verification Advisor</span><h2>What should happen next?</h2><p className="work-control-note">Deriving a deterministic recommendation…</p></section>;
  }
  if (error) {
    return <section className="work-control-card verification-advisor-card is-blocked" aria-label="Verification Advisor"><span className="eyebrow">Verification Advisor</span><h2>What should happen next?</h2><p className="work-control-note">Recommendation unavailable. {error}</p></section>;
  }
  if (!snapshot) {
    return <section className="work-control-card verification-advisor-card is-unknown" aria-label="Verification Advisor"><span className="eyebrow">Verification Advisor</span><h2>What should happen next?</h2><p className="work-control-note">Unknown — select a project and Work Item before RepoDesk can advise on verification.</p></section>;
  }

  const { recommendation } = snapshot;
  const action = decisionAction(recommendation.decision);
  const canAccept = recommendation.selected_check_ids.length > 0;

  return (
    <section className={`work-control-card verification-advisor-card is-${recommendation.decision}`} aria-label="Verification Advisor">
      <div className="work-control-card-heading">
        <div><span className="eyebrow">Verification Advisor</span><h2>What should happen next?</h2></div>
        <span className="work-control-state is-attention">{recommendation.uncertainty_label === "unknown" ? "Unknown confidence" : recommendation.decision}</span>
      </div>
      <p className="work-control-rationale">{recommendation.rationale[0] ?? "No rationale recorded."}</p>
      <button
        type="button"
        className="work-control-primary-action"
        onClick={() => onRecord(decisionInput(snapshot, recommendation.decision))}
        disabled={!canAccept}
      >
        {action} · {formatDuration(recommendation.estimated_wall_clock_ms)}
      </button>
      <div className="work-control-actions">
        <button type="button" className="secondary-cta" onClick={() => onRecord(decisionInput(snapshot, "defer_with_debt"))}>Defer with debt</button>
        <button type="button" className="link-cta" onClick={onInspect}>Inspect evidence</button>
      </div>
      <div className="work-control-facts" role="list" aria-label="Verification recommendation facts">
        <div role="listitem"><span>Selected checks</span><strong>{recommendation.selected_check_ids.length || "None"}</strong></div>
        <div role="listitem"><span>Estimated cost</span><strong>{recommendation.estimated_cost_units == null ? "Not measured" : `${recommendation.estimated_cost_units.toFixed(3)} units`}</strong></div>
        <div role="listitem"><span>Risk</span><strong>{recommendation.risk_label || "Unknown"}</strong></div>
        <div role="listitem"><span>Policy</span><strong>{recommendation.policy_version}</strong></div>
      </div>
      {recommendation.deferred_checks.length > 0 ? (
        <div className="work-control-debt" aria-label="Verification debt">
          <div className="work-control-debt-heading"><strong>Deferred</strong><span>{recommendation.deferred_checks.length} check{recommendation.deferred_checks.length === 1 ? "" : "s"}</span></div>
          <ul>{recommendation.deferred_checks.map((check) => <li key={check.check_id}><strong>{check.title}</strong><span>{check.reason}</span></li>)}</ul>
        </div>
      ) : null}
      <div className="work-control-source-list" aria-label="Evidence sources">
        {snapshot.sources.map((source) => <div key={source.source}><span>{source.source}</span><strong className={`is-${source.status}`}>{source.status}</strong></div>)}
      </div>
    </section>
  );
}
