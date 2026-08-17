import type { SemanticState } from "../../shared/ui/primitives";
import type { ProjectNoticeTone } from "./useProjectSetup";

export type ProjectWorkspaceState = "active" | "inactive";

function assertNever(value: never): never {
  throw new Error(`Unhandled Projects semantic state: ${String(value)}`);
}

export function projectWorkspaceSemantic(state: ProjectWorkspaceState): SemanticState {
  switch (state) {
    case "active":
      return { label: "Active", tone: "positive" };
    case "inactive":
      return { label: "No active project", tone: "neutral" };
    default:
      return assertNever(state);
  }
}

export function attributionPolicySemantic(required: boolean): SemanticState {
  return required
    ? { label: "Exact required", tone: "info" }
    : { label: "Informational", tone: "neutral" };
}

export function projectNoticeSemantic(tone: ProjectNoticeTone): SemanticState {
  switch (tone) {
    case "ok":
      return { label: "Complete", tone: "positive" };
    case "warn":
      return { label: "In progress", tone: "attention" };
    case "danger":
      return { label: "Failed", tone: "critical" };
    default:
      return assertNever(tone);
  }
}
