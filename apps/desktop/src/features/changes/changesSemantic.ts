import type {
  AcceptanceCriterionEvidence,
  AcceptanceEvidenceReport,
  ChangeAttributionEvidence,
  ChangeFileScopeState,
  ChangeGovernanceSnapshot,
  ChangeReviewState,
  CommitGateState,
  SafeCommitManifest,
  ScopeComplianceStatus,
} from "../../shared/api/engineering";
import type { SemanticState } from "../../shared/ui/primitives";

export type ChangeFileStatus = "staged" | "modified" | "untracked";

function assertNever(value: never): never {
  throw new Error(`Unhandled Changes semantic state: ${String(value)}`);
}

export function attributionSemantic(attribution: ChangeAttributionEvidence): SemanticState {
  switch (attribution.strength) {
    case "exact_isolated":
      return {
        label: "Exact · isolated worktree",
        tone: "positive",
        detail: attribution.workspace_id ? `Workspace ${attribution.workspace_id}` : "Managed isolated workspace",
      };
    case "exact_clean_workspace":
      return { label: "Exact · clean workspace", tone: "positive", detail: attribution.reason ?? "Exact workspace proof" };
    case "derived_pre_post":
      return { label: "Derived · pre/post", tone: "attention", detail: attribution.reason ?? "Derived producer evidence" };
    case "manual":
      return { label: "Manual handoff", tone: "neutral", detail: attribution.reason ?? "Human-imported changes" };
    case "unattributed":
      return { label: "Unattributed", tone: "critical", detail: attribution.reason ?? "No sufficient producer proof" };
    case "legacy_unknown":
      return { label: "Legacy / unknown", tone: "attention", detail: attribution.reason ?? "Historical evidence has no typed attribution" };
    default:
      return assertNever(attribution.strength);
  }
}

export function safeCommitSemantic(manifest: SafeCommitManifest): SemanticState {
  switch (manifest.state) {
    case "ready":
      return { label: "Ready to commit", tone: "positive" };
    case "committed":
      return { label: "Committed", tone: "positive" };
    case "blocked":
      return { label: "Commit blocked", tone: "critical", detail: manifest.blockers[0] };
    default:
      return assertNever(manifest.state);
  }
}

export function reviewSemantic(review: ChangeReviewState): SemanticState {
  switch (review) {
    case "accepted":
      return { label: "Accepted", tone: "positive" };
    case "rejected":
      return { label: "Rejected", tone: "critical" };
    case "proposed":
      return { label: "Proposed", tone: "attention" };
    default:
      return assertNever(review);
  }
}

export function verificationSemantic(governance: ChangeGovernanceSnapshot): SemanticState {
  const verification = governance.verification;
  switch (verification.state) {
    case "passed":
      if (verification.fresh === false) return { label: "Passed · stale", tone: "attention" };
      if (verification.fresh === true) return { label: "Passed · current", tone: "positive" };
      return { label: "Passed · unchecked", tone: "attention" };
    case "failed":
      return { label: "Failed", tone: "critical", detail: verification.error ?? undefined };
    case "running":
      return { label: "Running", tone: "info" };
    case "not_run":
      return { label: "Not run", tone: "neutral" };
    default:
      return assertNever(verification.state);
  }
}

export function gateSemantic(state: CommitGateState): SemanticState {
  switch (state) {
    case "ready":
      return { label: "Ready", tone: "positive" };
    case "committed":
      return { label: "Committed", tone: "positive" };
    case "verification_running":
      return { label: "Verification running", tone: "info" };
    case "no_change_set":
      return { label: "No ChangeSet", tone: "neutral" };
    case "needs_review":
      return { label: "Review required", tone: "attention" };
    case "verification_required":
      return { label: "Verification required", tone: "attention" };
    case "verification_stale":
      return { label: "Verification stale", tone: "attention" };
    case "scope_violation":
      return { label: "Scope violation", tone: "critical" };
    case "rejected":
      return { label: "Rejected", tone: "critical" };
    case "verification_failed":
      return { label: "Verification failed", tone: "critical" };
    default:
      return assertNever(state);
  }
}

export function scopeSemantic(status: ScopeComplianceStatus, overridden = false): SemanticState {
  if (overridden) return { label: "Overridden", tone: "attention" };
  switch (status) {
    case "compliant":
      return { label: "Compliant", tone: "positive" };
    case "violation":
      return { label: "Violation", tone: "critical" };
    case "unconfigured":
      return { label: "Unconfigured", tone: "attention" };
    case "not_evaluated":
      return { label: "Not evaluated", tone: "neutral" };
    default:
      return assertNever(status);
  }
}

export function fileScopeSemantic(state: ChangeFileScopeState): SemanticState {
  switch (state) {
    case "allowed":
      return { label: "In scope", tone: "positive" };
    case "out_of_scope":
      return { label: "Out of scope", tone: "critical" };
    case "protected":
      return { label: "Protected", tone: "critical" };
    case "ungoverned":
      return { label: "Ungoverned", tone: "attention" };
    default:
      return assertNever(state);
  }
}

export function fileStatusSemantic(status: ChangeFileStatus): SemanticState {
  switch (status) {
    case "staged":
      return { label: "S", tone: "positive", detail: "Staged" };
    case "modified":
      return { label: "M", tone: "attention", detail: "Modified" };
    case "untracked":
      return { label: "U", tone: "neutral", detail: "Untracked" };
    default:
      return assertNever(status);
  }
}

export function criterionSemantic(criterion: AcceptanceCriterionEvidence): SemanticState {
  if (criterion.stale) return { label: "Stale", tone: "attention", detail: criterion.stale_reason ?? undefined };
  switch (criterion.status) {
    case "proven":
      return { label: "Proven", tone: "positive" };
    case "failed":
      return { label: "Failed", tone: "critical" };
    case "unproven":
      return { label: "Unproven", tone: "attention" };
    default:
      return assertNever(criterion.status);
  }
}

export function acceptanceSemantic(acceptance: AcceptanceEvidenceReport): SemanticState {
  if (!acceptance.configured) return { label: "Not configured", tone: "neutral" };
  if (acceptance.failed > 0) return { label: "Failed evidence", tone: "critical", detail: `${acceptance.failed} failed` };
  if (acceptance.unproven > 0) return { label: "Incomplete", tone: "attention", detail: `${acceptance.unproven} unproven or stale` };
  return { label: "Complete", tone: "positive", detail: "All criteria evidenced" };
}

export function treeBindingSemantic(manifest: SafeCommitManifest): SemanticState {
  if (manifest.reviewed_tree_sha && manifest.verification_tree_sha === manifest.reviewed_tree_sha) {
    return { label: "Bound", tone: "positive", detail: "Matches verification tree" };
  }
  return { label: "Not bound", tone: "attention" };
}
