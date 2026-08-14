import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  WORK_ENGINEERING_SNAPSHOT_KEY,
  linkChangesAcceptanceEvidence,
  recordScopeOverride,
  type AcceptanceCriterionEvidence,
  type ChangeGovernanceSnapshot,
  type ChangeSetPassport,
  type SafeCommitManifest,
  type WorkEngineeringSnapshot,
} from "../../shared/api/engineering";
import { workVerify } from "../../shared/api/orchestrate";
import { errorToMessage } from "../../shared/utils/helpers";

function safeStateLabel(manifest: SafeCommitManifest): string {
  if (manifest.state === "ready") return "Ready to commit";
  if (manifest.state === "committed") return "Committed";
  return "Commit blocked";
}

function safeStateTone(manifest: SafeCommitManifest): string {
  if (manifest.state === "ready" || manifest.state === "committed") return "ok";
  return "danger";
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

function criterionTone(criterion: AcceptanceCriterionEvidence): string {
  if (criterion.status === "proven" && !criterion.stale) return "ok";
  if (criterion.status === "failed") return "danger";
  return "warn";
}

function criterionLabel(criterion: AcceptanceCriterionEvidence): string {
  if (criterion.stale) return "Stale";
  if (criterion.status === "proven") return "Proven";
  if (criterion.status === "failed") return "Failed";
  return "Unproven";
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
  manifest,
  loading,
  error,
}: {
  governance: ChangeGovernanceSnapshot | null;
  passport: ChangeSetPassport | null;
  manifest: SafeCommitManifest | null;
  loading: boolean;
  error: unknown;
}) {
  const queryClient = useQueryClient();
  const [showOverride, setShowOverride] = useState(false);
  const [overrideReason, setOverrideReason] = useState("");
  const [criterionCommands, setCriterionCommands] = useState<Record<string, string>>({});

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

  const linkEvidence = useMutation({
    mutationFn: ({ criterionId, command }: { criterionId: string; command: string }) =>
      linkChangesAcceptanceEvidence(criterionId, command),
    onSuccess: (snapshot) => {
      queryClient.setQueryData(WORK_ENGINEERING_SNAPSHOT_KEY, snapshot);
      refreshTrustState();
    },
  });

  if (error) {
    return <div className="notice danger">Change governance unavailable: {errorToMessage(error)}</div>;
  }
  if (loading || !governance || !passport || !manifest) {
    return <div className="change-evidence-loading">Loading ChangeSet evidence…</div>;
  }

  const changeset = governance.changeset_id ? governance.changeset_id.replace(/-changeset$/, "") : "none";
  const overridden = governance.scope_override != null;
  const canVerify = governance.gate.state === "verification_required"
    || governance.gate.state === "verification_failed"
    || governance.gate.state === "verification_stale";
  const acceptanceValue = manifest.acceptance.configured
    ? `${manifest.acceptance.proven}/${manifest.acceptance.criteria.length} proven`
    : "Not configured";
  const commands = manifest.verification_commands;
  const fallbackCommand = commands.find((command) => command.success)?.command ?? commands[0]?.command ?? "";
  const canLinkEvidence = governance.verification.state === "passed"
    && governance.verification.fresh === true
    && commands.length > 0
    && manifest.state !== "committed";

  const selectedCommand = (criterion: AcceptanceCriterionEvidence): string => {
    const explicit = criterionCommands[criterion.criterion_id];
    if (explicit && commands.some((command) => command.command === explicit)) return explicit;
    if (criterion.command && commands.some((command) => command.command === criterion.command)) return criterion.command;
    return fallbackCommand;
  };

  return (
    <section className="change-evidence" aria-label="ChangeSet evidence">
      <div className="change-evidence-head">
        <div>
          <p className="eyebrow">Safe Commit Manifest</p>
          <strong>{governance.changeset_id ? `ChangeSet ${changeset}` : "No recorded ChangeSet"}</strong>
          <small className="muted">Manifest {shortSha(manifest.manifest_digest)}</small>
        </div>
        <span className={`pill ${safeStateTone(manifest)}`}>{safeStateLabel(manifest)}</span>
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
          value={manifest.scope.overridden ? "Overridden" : manifest.scope.status.split("_").join(" ")}
          detail={`${manifest.reviewed_paths.length} reviewed path${manifest.reviewed_paths.length === 1 ? "" : "s"}`}
        />
        <EvidenceCell label="Review" value={reviewLabel(governance.review_state)} />
        <EvidenceCell
          label="Reviewed tree"
          value={shortSha(manifest.reviewed_tree_sha)}
          detail={manifest.verification_tree_sha === manifest.reviewed_tree_sha ? "Matches verification tree" : "Not bound"}
        />
        <EvidenceCell
          label="Verification"
          value={verificationLabel(governance)}
          detail={commands.length > 0 ? `${commands.length} canonical commands` : undefined}
        />
        <EvidenceCell
          label="Acceptance"
          value={acceptanceValue}
          detail={manifest.acceptance.failed > 0
            ? `${manifest.acceptance.failed} failed`
            : manifest.acceptance.unproven > 0
              ? `${manifest.acceptance.unproven} unproven or stale`
              : manifest.acceptance.configured ? "All criteria evidenced" : undefined}
        />
        <EvidenceCell
          label={manifest.state === "committed" ? "Commit" : "Current HEAD"}
          value={shortSha(manifest.commit_sha ?? manifest.current_head_sha)}
          detail={manifest.state === "committed" ? "Resulting tree is evidence-bound" : "Verification parent boundary"}
        />
      </div>

      {manifest.blockers.length > 0 ? (
        <div className="change-evidence-message danger">
          <strong>Commit blockers</strong>
          <ul className="change-evidence-list">
            {manifest.blockers.map((blocker) => <li key={blocker}>{blocker}</li>)}
          </ul>
        </div>
      ) : null}

      {manifest.warnings.length > 0 ? (
        <div className="change-evidence-message">
          <strong>Compatibility notes</strong>
          <ul className="change-evidence-list">
            {manifest.warnings.map((warning) => <li key={warning}>{warning}</li>)}
          </ul>
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

      {manifest.acceptance.configured ? (
        <div className="acceptance-matrix">
          <div className="acceptance-matrix-head">
            <div>
              <p className="eyebrow">Acceptance Evidence Matrix</p>
              <strong>{manifest.acceptance.proven}/{manifest.acceptance.criteria.length} criteria proven</strong>
            </div>
            <span className={`pill ${manifest.acceptance.failed > 0 ? "danger" : manifest.acceptance.unproven > 0 ? "warn" : "ok"}`}>
              {manifest.acceptance.failed > 0 ? "Failed evidence" : manifest.acceptance.unproven > 0 ? "Incomplete" : "Complete"}
            </span>
          </div>
          <div className="acceptance-matrix-rows">
            {manifest.acceptance.criteria.map((criterion) => {
              const command = selectedCommand(criterion);
              return (
                <div className="acceptance-matrix-row" key={criterion.criterion_id}>
                  <div className="acceptance-criterion-copy">
                    <span className={`pill ${criterionTone(criterion)}`}>{criterionLabel(criterion)}</span>
                    <strong>{criterion.criterion}</strong>
                    {criterion.command ? <code>{criterion.command}</code> : <small>No canonical command linked yet.</small>}
                    {criterion.stale_reason ? <small className="danger-text">{criterion.stale_reason}</small> : null}
                  </div>
                  {canLinkEvidence ? (
                    <div className="acceptance-link-control">
                      <select
                        value={command}
                        onChange={(event) => setCriterionCommands((current) => ({
                          ...current,
                          [criterion.criterion_id]: event.target.value,
                        }))}
                        aria-label={`Verification command for ${criterion.criterion}`}
                      >
                        {commands.map((item) => (
                          <option key={item.command} value={item.command}>
                            {item.success ? "PASS" : "FAIL"} · {item.command}
                          </option>
                        ))}
                      </select>
                      <button
                        type="button"
                        className="tiny-button"
                        disabled={!command || linkEvidence.isPending}
                        onClick={() => linkEvidence.mutate({ criterionId: criterion.criterion_id, command })}
                      >
                        {criterion.command ? "Relink evidence" : "Link evidence"}
                      </button>
                    </div>
                  ) : null}
                </div>
              );
            })}
          </div>
          {manifest.acceptance.criteria.length === 0 ? (
            <p className="muted">No acceptance criteria are configured for this Work Item.</p>
          ) : null}
          {linkEvidence.isError ? <div className="notice danger">{errorToMessage(linkEvidence.error)}</div> : null}
        </div>
      ) : null}

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
