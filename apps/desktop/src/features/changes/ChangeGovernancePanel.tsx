import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  WORK_ENGINEERING_SNAPSHOT_KEY,
  recordScopeOverride,
  type ChangeGovernanceSnapshot,
  type ChangeSetPassport,
  type CommitGateState,
  type WorkEngineeringSnapshot,
} from "../../shared/api/engineering";
import { workVerify } from "../../shared/api/orchestrate";
import { errorToMessage } from "../../shared/utils/helpers";

function gateLabel(state: CommitGateState): string {
  switch (state) {
    case "no_change_set": return "No ChangeSet";
    case "scope_violation": return "Scope blocked";
    case "needs_review": return "Needs review";
    case "rejected": return "Rejected";
    case "verification_required": return "Needs verification";
    case "verification_running": return "Verifying";
    case "verification_failed": return "Verification failed";
    case "verification_stale": return "Verification stale";
    case "ready": return "Ready to commit";
    case "committed": return "Committed";
  }
}

function gateTone(state: CommitGateState): string {
  switch (state) {
    case "ready":
    case "committed":
      return "ok";
    case "scope_violation":
    case "rejected":
    case "verification_failed":
    case "verification_stale":
      return "danger";
    case "needs_review":
    case "verification_required":
    case "verification_running":
      return "warn";
    default:
      return "neutral";
  }
}

function reviewLabel(value: ChangeGovernanceSnapshot["review_state"]): string {
  if (value === "accepted") return "Accepted";
  if (value === "rejected") return "Rejected";
  return "Proposed";
}

function verificationLabel(governance: ChangeGovernanceSnapshot): string {
  const value = governance.verification.state;
  if (value === "passed" && governance.verification.fresh === false) return "Passed · stale";
  if (value === "passed" && governance.verification.fresh === true) return "Passed · current";
  if (value === "passed") return "Passed · unchecked";
  if (value === "failed") return "Failed";
  if (value === "running") return "Running";
  return "Not run";
}

function workerLabel(governance: ChangeGovernanceSnapshot): string {
  if (governance.origin.workers.length === 0) return "Unattributed";
  return governance.origin.workers
    .slice(0, 2)
    .map((worker) => worker.id)
    .join(" + ");
}

function attributionLabel(passport: ChangeSetPassport): string {
  if (passport.attribution === "recorded_run") return "Recorded run";
  if (passport.attribution === "manual") return "Manual handoff";
  return "Unattributed";
}

function shortSha(value: string | null): string {
  if (!value) return "—";
  return value.slice(0, Math.min(value.length, 12));
}

function EvidenceCell({ label, value, detail }: { label: string; value: string; detail?: string }) {
  return (
    <div className="change-evidence-cell">
      <span>{label}</span>
      <strong>{value}</strong>
      {detail ? <small>{detail}</small> : null}
    </div>
  );
}

export function ChangeGovernancePanel({
  governance,
  passport,
  loading,
  error,
}: {
  governance: ChangeGovernanceSnapshot | null;
  passport: ChangeSetPassport | null;
  loading: boolean;
  error: unknown;
}) {
  const queryClient = useQueryClient();
  const [showOverride, setShowOverride] = useState(false);
  const [overrideReason, setOverrideReason] = useState("");

  const refreshTrustState = () => {
    void queryClient.invalidateQueries({ queryKey: WORK_ENGINEERING_SNAPSHOT_KEY });
    void queryClient.invalidateQueries({ queryKey: ["work"] });
    void queryClient.invalidateQueries({ queryKey: ["git"] });
  };

  const override = useMutation({
    mutationFn: (reason: string) => recordScopeOverride(reason),
    onSuccess: (next) => {
      queryClient.setQueryData<WorkEngineeringSnapshot>(WORK_ENGINEERING_SNAPSHOT_KEY, (current) =>
        current ? { ...current, change_governance: next } : current,
      );
      setOverrideReason("");
      setShowOverride(false);
      refreshTrustState();
    },
  });

  const verify = useMutation({
    mutationFn: workVerify,
    onSuccess: refreshTrustState,
  });

  if (error) {
    return <div className="notice danger">Change governance unavailable: {errorToMessage(error)}</div>;
  }
  if (loading || !governance || !passport) {
    return <div className="change-evidence-loading">Loading ChangeSet evidence…</div>;
  }

  const changeset = governance.changeset_id ? governance.changeset_id.replace(/-changeset$/, "") : "none";
  const overridden = governance.scope_override != null;
  const canVerify = governance.gate.state === "verification_required"
    || governance.gate.state === "verification_failed"
    || governance.gate.state === "verification_stale";
  const acceptanceValue = passport.acceptance.configured
    ? `${passport.acceptance.proven}/${passport.acceptance.total} proven`
    : "Not configured";

  return (
    <section className="change-evidence" aria-label="ChangeSet evidence">
      <div className="change-evidence-head">
        <div>
          <p className="eyebrow">ChangeSet Passport</p>
          <strong>{governance.changeset_id ? `ChangeSet ${changeset}` : "No recorded ChangeSet"}</strong>
        </div>
        <span className={`pill ${gateTone(governance.gate.state)}`}>{gateLabel(governance.gate.state)}</span>
      </div>

      <div className="change-evidence-grid">
        <EvidenceCell
          label="Origin"
          value={workerLabel(governance)}
          detail={`${attributionLabel(passport)} · ${governance.origin.execution_mode ?? "no execution mode"}`}
        />
        <EvidenceCell
          label="Baseline"
          value={shortSha(passport.baseline_commit)}
          detail={passport.run_id ? `Run ${passport.run_id}` : "No canonical run receipt"}
        />
        <EvidenceCell
          label="Scope"
          value={overridden ? "Overridden" : governance.scope_status.split("_").join(" ")}
          detail={`${passport.changed_file_count} recorded file${passport.changed_file_count === 1 ? "" : "s"}`}
        />
        <EvidenceCell label="Review" value={reviewLabel(governance.review_state)} />
        <EvidenceCell
          label="Verification"
          value={verificationLabel(governance)}
          detail={governance.verification.command_count > 0 ? `${governance.verification.command_count} canonical commands` : undefined}
        />
        <EvidenceCell
          label="Acceptance"
          value={acceptanceValue}
          detail={passport.acceptance.failed > 0
            ? `${passport.acceptance.failed} failed`
            : passport.acceptance.unproven > 0
              ? `${passport.acceptance.unproven} unproven`
              : passport.acceptance.configured ? "All criteria evidenced" : undefined}
        />
      </div>

      {governance.gate.blockers.length > 0 ? (
        <div className="change-evidence-message danger">
          <strong>Blocked</strong>
          <span>{governance.gate.blockers[0]}</span>
        </div>
      ) : null}

      {governance.gate.warnings.length > 0 ? (
        <div className="change-evidence-message">
          <strong>Note</strong>
          <span>{governance.gate.warnings[0]}</span>
        </div>
      ) : null}

      {governance.verification.stale_reason ? (
        <div className="change-evidence-message danger">
          <strong>Stale receipt</strong>
          <span>{governance.verification.stale_reason}</span>
        </div>
      ) : null}

      {canVerify ? (
        <div className="change-evidence-actions">
          <button
            type="button"
            className="primary-button"
            disabled={verify.isPending}
            onClick={() => verify.mutate()}
          >
            {verify.isPending ? "Verifying reviewed ChangeSet…" : "Verify reviewed ChangeSet"}
          </button>
          <span className="muted">Creates a canonical receipt bound to the current HEAD, accepted index tree and ChangeSet.</span>
        </div>
      ) : null}
      {verify.isError ? <div className="notice danger">{errorToMessage(verify.error)}</div> : null}

      {governance.scope_override ? (
        <div className="change-override-receipt">
          <span className="pill warn">Human override</span>
          <span>{governance.scope_override.reason}</span>
        </div>
      ) : governance.gate.state === "scope_violation" ? (
        !showOverride ? (
          <div className="change-evidence-actions">
            <button type="button" className="ghost-button" onClick={() => setShowOverride(true)}>
              Record one-time override
            </button>
            <span className="muted">The exception applies only to this ChangeSet and becomes stale if the contract changes.</span>
          </div>
        ) : (
          <div className="change-override-editor">
            <label htmlFor="scope-override-reason">Why is this contract exception acceptable?</label>
            <input
              id="scope-override-reason"
              value={overrideReason}
              onChange={(event) => setOverrideReason(event.target.value)}
              placeholder="Example: README update is required by this implementation"
              maxLength={500}
              autoFocus
            />
            {override.isError ? <div className="notice danger">{errorToMessage(override.error)}</div> : null}
            <div className="change-evidence-actions">
              <button
                type="button"
                className="primary-button"
                disabled={override.isPending || overrideReason.trim().length === 0}
                onClick={() => override.mutate(overrideReason.trim())}
              >
                {override.isPending ? "Recording…" : "Record override"}
              </button>
              <button type="button" className="ghost-button" disabled={override.isPending} onClick={() => setShowOverride(false)}>
                Cancel
              </button>
            </div>
          </div>
        )
      ) : null}
    </section>
  );
}
