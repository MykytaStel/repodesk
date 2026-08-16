import type {
  ExecutionEvidenceStatus,
  PhaseProgress,
  PhaseStatus,
} from "../../shared/api/orchestrate";
import type { SemanticState } from "../../shared/ui/primitives";

type LaunchApprovalState = "ready" | "action_required" | "stale";

function assertNever(value: never): never {
  throw new Error(`Unhandled Work semantic state: ${String(value)}`);
}

export function phaseStatusSemantic(status: PhaseStatus): SemanticState {
  switch (status) {
    case "done":
      return { label: "Done", tone: "positive" };
    case "in_progress":
      return { label: "Current", tone: "info" };
    case "available":
      return { label: "Available", tone: "attention" };
    case "locked":
      return { label: "Locked", tone: "neutral" };
    default:
      return assertNever(status);
  }
}

export function workflowPositionSemantic(progress: PhaseProgress): SemanticState {
  if (progress.complete) {
    return { label: "Workflow complete", tone: "positive", detail: "All canonical Work phases are complete." };
  }
  const current = progress.phases.find((phase) => phase.phase === progress.current);
  const semantic = current ? phaseStatusSemantic(current.status) : { label: "Current", tone: "info" as const };
  return {
    label: current?.title ?? progress.current,
    tone: semantic.tone,
    detail: current?.summary,
  };
}

export function executionEvidenceSemantic(status: ExecutionEvidenceStatus): SemanticState {
  switch (status) {
    case "ready":
      return { label: "Evidence ready", tone: "positive" };
    case "recovery_required":
      return { label: "Recovery required", tone: "attention" };
    case "incomplete":
      return { label: "Evidence incomplete", tone: "critical" };
    case "not_required":
      return { label: "Evidence not required", tone: "neutral" };
    default:
      return assertNever(status);
  }
}

export function packetPreparationSemantic(prepared: boolean): SemanticState {
  return prepared
    ? { label: "Prepared", tone: "positive", detail: "Prepared context is bound to this execution packet." }
    : { label: "Rebuild required", tone: "attention", detail: "Context will be rebuilt before launch." };
}

export function launchApprovalSemantic(input: {
  stale: boolean;
  ready: boolean;
}): SemanticState {
  const state: LaunchApprovalState = input.stale
    ? "stale"
    : input.ready
      ? "ready"
      : "action_required";

  switch (state) {
    case "ready":
      return { label: "Ready", tone: "positive", detail: "All capabilities required by this exact plan lock are approved." };
    case "action_required":
      return { label: "Action required", tone: "attention", detail: "Approve the capabilities required by this exact plan lock." };
    case "stale":
      return { label: "Approval stale", tone: "attention", detail: "The execution packet changed after approval." };
    default:
      return assertNever(state);
  }
}
