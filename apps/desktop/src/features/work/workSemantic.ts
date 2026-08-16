import type {
  ExecutionEvidenceStatus,
  PhaseStatus,
} from "../../shared/api/orchestrate";
import type { SemanticState } from "../../shared/ui/primitives";

function assertNever(value: never): never {
  throw new Error(`Unhandled Work semantic state: ${String(value)}`);
}

export function phaseSemanticState(status: PhaseStatus, current: boolean): SemanticState {
  if (current) return { label: "Current", tone: "info" };

  switch (status) {
    case "done":
      return { label: "Done", tone: "positive" };
    case "in_progress":
      return { label: "In progress", tone: "info" };
    case "available":
      return { label: "Available", tone: "neutral" };
    case "locked":
      return { label: "Locked", tone: "neutral" };
    default:
      return assertNever(status);
  }
}

export function executionEvidenceSemanticState(
  status: ExecutionEvidenceStatus,
): SemanticState {
  switch (status) {
    case "ready":
      return { label: "Complete", tone: "positive" };
    case "recovery_required":
      return { label: "Recovery required", tone: "attention" };
    case "incomplete":
      return { label: "Incomplete", tone: "critical" };
    case "not_required":
      return { label: "Not required", tone: "neutral" };
    default:
      return assertNever(status);
  }
}

export function preparedContextSemanticState(prepared: boolean): SemanticState {
  return prepared
    ? { label: "Prepared", tone: "positive" }
    : { label: "Rebuild required", tone: "attention" };
}

export function approvalSemanticState(ready: boolean): SemanticState {
  return ready
    ? { label: "Ready", tone: "positive" }
    : { label: "Action required", tone: "attention" };
}
