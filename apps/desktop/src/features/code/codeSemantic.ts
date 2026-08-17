import type { CodeWorkspaceFileStatus } from "../../shared/api/codeWorkspace";
import type {
  ChangeFileScopeState,
  ChangeReviewState,
  ChangeVerificationState,
} from "../../shared/api/engineering";
import type { RepositoryEvidenceLevel } from "../../shared/api/repositoryIntelligence";
import type { SemanticState } from "../../shared/ui/primitives";
import type { SemanticOrigin } from "./useSemanticCodeState";

export type CodeSaveState = "saved" | "dirty" | "saving";

function assertNever(value: never): never {
  throw new Error(`Unhandled Code semantic state: ${String(value)}`);
}

export function codeFileStatusSemantic(status: CodeWorkspaceFileStatus): SemanticState {
  switch (status) {
    case "clean":
      return { label: "Clean", tone: "neutral", detail: "Clean working-tree state" };
    case "modified":
      return { label: "M", tone: "attention", detail: "Modified" };
    case "added":
      return { label: "A", tone: "positive", detail: "Added" };
    case "deleted":
      return { label: "D", tone: "critical", detail: "Deleted" };
    case "untracked":
      return { label: "U", tone: "neutral", detail: "Untracked" };
    case "renamed":
      return { label: "R", tone: "info", detail: "Renamed" };
    case "conflict":
      return { label: "!", tone: "critical", detail: "Conflict" };
    default:
      return assertNever(status);
  }
}

export function codeScopeSemantic(state: ChangeFileScopeState): SemanticState {
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

export function codeReviewSemantic(state: ChangeReviewState): SemanticState {
  switch (state) {
    case "accepted":
      return { label: "Accepted", tone: "positive" };
    case "rejected":
      return { label: "Rejected", tone: "critical" };
    case "proposed":
      return { label: "Proposed", tone: "attention" };
    default:
      return assertNever(state);
  }
}

export function codeVerificationSemantic(state: ChangeVerificationState, dirty: boolean): SemanticState {
  switch (state) {
    case "passed":
      return dirty
        ? { label: "Draft after verification", tone: "attention", detail: "Editor content changed after the last passing verification" }
        : { label: "Verified", tone: "positive" };
    case "failed":
      return { label: "Verification failed", tone: "critical" };
    case "running":
      return { label: "Verifying", tone: "info" };
    case "not_run":
      return { label: "Not verified", tone: "neutral" };
    default:
      return assertNever(state);
  }
}

export function codeOriginSemantic(origin: SemanticOrigin, detail?: string | null): SemanticState {
  switch (origin) {
    case "human":
      return { label: "Human", tone: "neutral", detail: detail ?? undefined };
    case "agent":
      return { label: "AI worker", tone: "info", detail: detail ?? undefined };
    case "mixed":
      return { label: "Human + AI", tone: "info", detail: detail ?? undefined };
    case "automation":
      return { label: "Automation", tone: "neutral", detail: detail ?? undefined };
    case "unknown":
      return { label: "Unknown origin", tone: "neutral", detail: detail ?? undefined };
    default:
      return assertNever(origin);
  }
}

export function repositoryEvidenceSemantic(level: RepositoryEvidenceLevel): SemanticState {
  switch (level) {
    case "strong":
      return { label: "Strong", tone: "positive" };
    case "bounded":
      return { label: "Bounded", tone: "attention" };
    case "unavailable":
      return { label: "Unavailable", tone: "neutral" };
    default:
      return assertNever(level);
  }
}

export function codeWorkspaceIndexSemantic(truncated: boolean): SemanticState {
  return truncated
    ? { label: "Index capped", tone: "attention", detail: "Repository index reached a bounded safety or size limit" }
    : { label: "Indexed", tone: "positive" };
}

export function codeSaveSemantic(state: CodeSaveState): SemanticState {
  switch (state) {
    case "saved":
      return { label: "Saved", tone: "positive" };
    case "dirty":
      return { label: "Unsaved", tone: "attention" };
    case "saving":
      return { label: "Saving", tone: "info" };
    default:
      return assertNever(state);
  }
}
