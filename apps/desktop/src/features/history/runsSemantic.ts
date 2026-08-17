import type { AcceptanceCriterionStatus } from "../../shared/api/engineering";
import type { RunStatus, SubAgentStatus } from "../../shared/api/orchestrate";
import type { RunDispositionState } from "../../shared/api/observability";
import type { SemanticState } from "../../shared/ui/primitives";

export type RunReviewState = "proposed" | "accepted" | "rejected" | "unknown";
export type RunVerificationState = "not_run" | "running" | "passed" | "failed" | "stale" | "unknown";

function assertNever(value: never): never {
  throw new Error(`Unhandled Runs semantic state: ${String(value)}`);
}

export function normalizeRunReviewState(value: string): RunReviewState {
  switch (value) {
    case "proposed":
    case "accepted":
    case "rejected":
    case "unknown":
      return value;
    default:
      return "unknown";
  }
}

export function normalizeRunVerificationState(value: string): RunVerificationState {
  switch (value) {
    case "not_run":
    case "running":
    case "passed":
    case "failed":
    case "stale":
    case "unknown":
      return value;
    default:
      return "unknown";
  }
}

export function runStatusSemantic(status: RunStatus): SemanticState {
  switch (status) {
    case "completed":
      return { label: "completed", tone: "positive" };
    case "partial":
      return { label: "partial", tone: "attention" };
    case "failed":
      return { label: "failed", tone: "critical" };
    case "dry_run":
      return { label: "dry run", tone: "neutral" };
    default:
      return assertNever(status);
  }
}

export function workerStatusSemantic(status: SubAgentStatus): SemanticState {
  switch (status) {
    case "ok":
      return { label: "ok", tone: "positive" };
    case "skipped":
      return { label: "skipped", tone: "neutral" };
    case "blocked":
      return { label: "blocked", tone: "critical" };
    case "failed":
      return { label: "failed", tone: "critical" };
    default:
      return assertNever(status);
  }
}

export function reviewStateSemantic(status: RunReviewState): SemanticState {
  switch (status) {
    case "accepted":
      return { label: "accepted", tone: "positive" };
    case "rejected":
      return { label: "rejected", tone: "critical" };
    case "proposed":
      return { label: "proposed", tone: "attention" };
    case "unknown":
      return { label: "unknown", tone: "neutral" };
    default:
      return assertNever(status);
  }
}

export function verificationStateSemantic(status: RunVerificationState): SemanticState {
  switch (status) {
    case "passed":
      return { label: "passed", tone: "positive" };
    case "failed":
      return { label: "failed", tone: "critical" };
    case "running":
      return { label: "running", tone: "info" };
    case "stale":
      return { label: "stale", tone: "attention" };
    case "not_run":
      return { label: "not run", tone: "neutral" };
    case "unknown":
      return { label: "unknown", tone: "neutral" };
    default:
      return assertNever(status);
  }
}

export function acceptanceCriterionSemantic(
  status: AcceptanceCriterionStatus,
  stale: boolean,
): SemanticState {
  if (stale) return { label: "Stale", tone: "attention" };
  switch (status) {
    case "proven":
      return { label: "PROVEN", tone: "positive" };
    case "failed":
      return { label: "FAILED", tone: "critical" };
    case "unproven":
      return { label: "UNPROVEN", tone: "attention" };
    default:
      return assertNever(status);
  }
}

export function runDispositionSemantic(status: RunDispositionState): SemanticState {
  switch (status) {
    case "complete":
      return { label: "complete", tone: "positive" };
    case "ready":
      return { label: "ready", tone: "info" };
    case "attention":
      return { label: "attention", tone: "attention" };
    case "blocked":
      return { label: "blocked", tone: "critical" };
    default:
      return assertNever(status);
  }
}

export function commitSemantic(committed: boolean): SemanticState {
  return committed
    ? { label: "committed", tone: "positive" }
    : { label: "not committed", tone: "neutral" };
}

export function verificationCommandSemantic(success: boolean): SemanticState {
  return success
    ? { label: "passed", tone: "positive" }
    : { label: "failed", tone: "critical" };
}
