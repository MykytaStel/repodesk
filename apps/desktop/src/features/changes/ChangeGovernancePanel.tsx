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
import {
  ActionBar,
  ErrorState,
  EvidenceState,
  LoadingState,
  PanelHeader,
  StatusBadge,
} from "../../shared/ui/primitives";
import {
  acceptanceSemantic,
  attributionSemantic,
  criterionSemantic,
  reviewSemantic,
  safeCommitSemantic,
  scopeSemantic,
  treeBindingSemantic,
  verificationSemantic,
} from "./changesSemantic";

function workerLabel(governance: ChangeGovernanceSnapshot): string {
  if (governance.origin.workers.length === 0) return "Unattributed";
  return governance.origin.workers
    .slice(0, 2)
    .map((worker) => worker.id)
    .join(" + ");
}

function shortSha(value: string | null): string {
  if (!value) return "—";
  return value.slice(0, Math.min(value.length, 12));
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
    return <ErrorState title="Change governance unavailable" detail={errorToMessage(error)} />;
  }
  if (loading || !governance || !passport || !manifest) {
    return <LoadingState message="Loading ChangeSet evidence…" />;
  }

  const changeset = governance.changeset_id ? governance.changeset_id.replace(/-changeset$/, "") : "none";
  const overridden = governance.scope_override != null;
  const canVerify = governance.gate.state === "verification_required"
    || governance.gate.state === "verification_failed"
    || governance.gate.state === "verification_stale";
  const commands = manifest.verification_commands;
  const fallbackCommand = commands.find((command) => command.success)?.command ?? commands[0]?.command ?? "";
  const canLinkEvidence = governance.verification.state === "passed"
    && governance.verification.fresh === true
    && commands.length > 0
    && manifest.state !== "committed";

  const attribution = attributionSemantic(manifest.attribution);
  const safeCommit = safeCommitSemantic(manifest);
  const scope = scopeSemantic(manifest.scope.status, manifest.scope.overridden);
  const review = reviewSemantic(governance.review_state);
  const verification = verificationSemantic(governance);
  const acceptance = acceptanceSemantic(manifest.acceptance);
  const treeBinding = treeBindingSemantic(manifest);

  const selectedCommand = (criterion: AcceptanceCriterionEvidence): string => {
    const explicit = criterionCommands[criterion.criterion_id];
    if (explicit && commands.some((command) => command.command === explicit)) return explicit;
    if (criterion.command && commands.some((command) => command.command === criterion.command)) return criterion.command;
    return fallbackCommand;
  };

  return (
    <section className="change-evidence" aria-label="ChangeSet evidence">
      <PanelHeader
        eyebrow="Safe Commit Manifest"
        title={governance.changeset_id ? `ChangeSet ${changeset}` : "No recorded ChangeSet"}
        description={`Manifest ${shortSha(manifest.manifest_digest)}`}
        trailing={<StatusBadge label={safeCommit.label} tone={safeCommit.tone} />}
      />

      <div className="change-evidence-grid">
        <EvidenceState
          label="Producer attribution"
          state={attribution.label}
          tone={attribution.tone}
          detail={`${attribution.detail ?? ""}${manifest.exact_attribution_required ? `${attribution.detail ? " · " : ""}exact required by project` : ""}`}
        />
        <EvidenceState
          label="Origin"
          state={workerLabel(governance)}
          tone={governance.origin.workers.length > 0 ? "info" : "critical"}
          detail={governance.origin.execution_mode ?? "no execution mode"}
        />
        <EvidenceState
          label="Baseline"
          state={shortSha(passport.baseline_commit ?? manifest.attribution.baseline_commit)}
          tone="neutral"
          detail={passport.run_id ? `Run ${passport.run_id}` : "No canonical run receipt"}
        />
        <EvidenceState
          label="Scope"
          state={scope.label}
          tone={scope.tone}
          detail={`${manifest.reviewed_paths.length} reviewed path${manifest.reviewed_paths.length === 1 ? "" : "s"}`}
        />
        <EvidenceState label="Review" state={review.label} tone={review.tone} />
        <EvidenceState
          label="Reviewed tree"
          state={shortSha(manifest.reviewed_tree_sha)}
          tone={treeBinding.tone}
          detail={treeBinding.detail ?? treeBinding.label}
        />
        <EvidenceState
          label="Verification"
          state={verification.label}
          tone={verification.tone}
          detail={verification.detail ?? (commands.length > 0 ? `${commands.length} canonical commands` : undefined)}
        />
        <EvidenceState
          label="Acceptance"
          state={manifest.acceptance.configured
            ? `${manifest.acceptance.proven}/${manifest.acceptance.criteria.length} proven`
            : acceptance.label}
          tone={acceptance.tone}
          detail={acceptance.detail}
        />
        <EvidenceState
          label={manifest.state === "committed" ? "Commit" : "Current HEAD"}
          state={shortSha(manifest.commit_sha ?? manifest.current_head_sha)}
          tone={manifest.state === "committed" ? "positive" : "neutral"}
          detail={manifest.state === "committed" ? "Resulting tree is evidence-bound" : "Verification parent boundary"}
        />
      </div>

      {manifest.blockers.length > 0 ? (
        <ErrorState
          title="Commit blockers"
          detail={(
            <ul className="change-evidence-list">
              {manifest.blockers.map((blocker) => <li key={blocker}>{blocker}</li>)}
            </ul>
          )}
        />
      ) : null}

      {manifest.warnings.length > 0 ? (
        <div className="change-evidence-message" data-semantic-tone="attention">
          <strong>Compatibility notes</strong>
          <ul className="change-evidence-list">
            {manifest.warnings.map((warning) => <li key={warning}>{warning}</li>)}
          </ul>
        </div>
      ) : null}

      {governance.verification.stale_reason ? (
        <EvidenceState
          label="Stale receipt"
          state="Stale"
          tone="attention"
          detail={governance.verification.stale_reason}
        />
      ) : null}

      {canVerify ? (
        <ActionBar
          primary={(
            <button
              type="button"
              className="primary-button"
              disabled={verify.isPending}
              onClick={() => verify.mutate()}
            >
              {verify.isPending ? "Verifying reviewed ChangeSet…" : "Verify reviewed ChangeSet"}
            </button>
          )}
          detail="Creates a canonical receipt bound to the current HEAD, accepted index tree and ChangeSet."
        />
      ) : null}
      {verify.isError ? <ErrorState title="Verification failed to start" detail={errorToMessage(verify.error)} /> : null}

      {manifest.acceptance.configured ? (
        <div className="acceptance-matrix">
          <PanelHeader
            eyebrow="Acceptance Evidence Matrix"
            title={`${manifest.acceptance.proven}/${manifest.acceptance.criteria.length} criteria proven`}
            trailing={<StatusBadge label={acceptance.label} tone={acceptance.tone} />}
          />
          <div className="acceptance-matrix-rows">
            {manifest.acceptance.criteria.map((criterion) => {
              const command = selectedCommand(criterion);
              const semantic = criterionSemantic(criterion);
              return (
                <div className="acceptance-matrix-row" key={criterion.criterion_id}>
                  <div className="acceptance-criterion-copy">
                    <StatusBadge label={semantic.label} tone={semantic.tone} />
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
          {linkEvidence.isError ? <ErrorState title="Could not link acceptance evidence" detail={errorToMessage(linkEvidence.error)} /> : null}
        </div>
      ) : null}

      {governance.scope_override ? (
        <div className="change-override-receipt">
          <StatusBadge label="Human override" tone="attention" />
          <span>{governance.scope_override.reason}</span>
        </div>
      ) : governance.gate.state === "scope_violation" ? (
        !showOverride ? (
          <ActionBar
            secondary={(
              <button type="button" className="ghost-button" onClick={() => setShowOverride(true)}>
                Record one-time override
              </button>
            )}
            detail="The exception applies only to this ChangeSet and becomes stale if the contract changes."
          />
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
            {override.isError ? <ErrorState title="Could not record override" detail={errorToMessage(override.error)} /> : null}
            <ActionBar
              primary={(
                <button
                  type="button"
                  className="primary-button"
                  disabled={override.isPending || overrideReason.trim().length === 0}
                  onClick={() => override.mutate(overrideReason.trim())}
                >
                  {override.isPending ? "Recording…" : "Record override"}
                </button>
              )}
              secondary={(
                <button type="button" className="ghost-button" disabled={override.isPending} onClick={() => setShowOverride(false)}>
                  Cancel
                </button>
              )}
            />
          </div>
        )
      ) : null}
    </section>
  );
}
