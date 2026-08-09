import { useQuery } from "@tanstack/react-query";
import { useMemo, useSyncExternalStore } from "react";
import {
  WORK_ENGINEERING_SNAPSHOT_KEY,
  workEngineeringSnapshot,
  type ChangeFileScopeState,
  type ChangeReviewState,
  type ChangeVerificationState,
  type WorkerRef,
} from "../../shared/api/engineering";
import { getProblemSnapshot, subscribeProblems, type ProblemDiagnostic } from "../../shared/api/problems";
import { callCommand } from "../../shared/api/queries";
import type { CodeWorkspaceFileStatus } from "../../shared/api/codeWorkspace";

export type GitLineKind = "added" | "modified" | "deleted";

export type GitLineMarker = {
  line: number;
  kind: GitLineKind;
};

export type SemanticOrigin = "human" | "agent" | "mixed" | "automation" | "unknown";

export type SemanticFileState = {
  path: string;
  workItemId: string | null;
  scopeState: ChangeFileScopeState | null;
  reviewState: ChangeReviewState | null;
  verificationState: ChangeVerificationState | null;
  gateState: string | null;
  gateReady: boolean;
  origin: SemanticOrigin;
  originLabel: string | null;
  problems: readonly ProblemDiagnostic[];
  errors: number;
  warnings: number;
  gitLines: readonly GitLineMarker[];
};

const GIT_DIFF_KEY = ["git", "semantic-file-diff"] as const;

function markerPriority(kind: GitLineKind): number {
  if (kind === "deleted") return 3;
  if (kind === "modified") return 2;
  return 1;
}

function putMarker(markers: Map<number, GitLineKind>, line: number, kind: GitLineKind) {
  const safeLine = Math.max(1, line);
  const current = markers.get(safeLine);
  if (!current || markerPriority(kind) > markerPriority(current)) markers.set(safeLine, kind);
}

/**
 * Project a unified Git diff onto current-document line numbers. This is a
 * visual hint only; Git remains authoritative and Changes owns full review.
 * Deletions have no current line, so their marker is anchored to the next
 * surviving line (or the preceding line at EOF).
 */
export function parseGitLineMarkers(diff: string): GitLineMarker[] {
  const markers = new Map<number, GitLineKind>();
  let newLine = 0;
  let pendingDeletes = 0;
  let inHunk = false;

  const flushDeletes = () => {
    if (pendingDeletes <= 0) return;
    putMarker(markers, Math.max(1, newLine), "deleted");
    pendingDeletes = 0;
  };

  for (const raw of diff.split(/\r?\n/)) {
    const hunk = raw.match(/^@@\s+-\d+(?:,\d+)?\s+\+(\d+)(?:,\d+)?\s+@@/);
    if (hunk) {
      flushDeletes();
      newLine = Number(hunk[1]);
      inHunk = true;
      continue;
    }
    if (!inHunk) continue;
    if (raw.startsWith("\\ No newline")) continue;

    const prefix = raw[0];
    if (prefix === "-") {
      pendingDeletes += 1;
      continue;
    }
    if (prefix === "+") {
      const kind: GitLineKind = pendingDeletes > 0 ? "modified" : "added";
      putMarker(markers, newLine, kind);
      if (pendingDeletes > 0) pendingDeletes -= 1;
      newLine += 1;
      continue;
    }
    if (prefix === " ") {
      flushDeletes();
      newLine += 1;
      continue;
    }

    // A new file header or malformed tail ends the current hunk projection.
    flushDeletes();
    inHunk = false;
  }
  flushDeletes();

  return [...markers.entries()]
    .map(([line, kind]) => ({ line, kind }))
    .sort((left, right) => left.line - right.line);
}

function originFromWorkers(workers: WorkerRef[]): { origin: SemanticOrigin; label: string | null } {
  if (workers.length === 0) return { origin: "unknown", label: null };
  const kinds = new Set(workers.map((worker) => worker.kind));
  const human = kinds.has("human") || kinds.has("manual");
  const agent = kinds.has("coding_agent") || kinds.has("inference");
  const automation = [...kinds].some((kind) => kind === "script" || kind === "ci" || kind === "check_runner");

  const labels = workers
    .map((worker) => worker.model || worker.id || worker.provider)
    .filter((value): value is string => Boolean(value && value.trim()))
    .slice(0, 2);
  const label = labels.length > 0 ? labels.join(" + ") : null;

  if (human && agent) return { origin: "mixed", label: label ?? "Human + agent" };
  if (agent) return { origin: "agent", label: label ?? "AI worker" };
  if (human) return { origin: "human", label: label ?? "Human" };
  if (automation) return { origin: "automation", label: label ?? "Automation" };
  return { origin: "unknown", label };
}

export function useSemanticCodeState({
  projectName,
  path,
  status,
  dirty,
}: {
  projectName: string | null | undefined;
  path: string;
  status: CodeWorkspaceFileStatus;
  dirty: boolean;
}): SemanticFileState {
  const problems = useSyncExternalStore(subscribeProblems, getProblemSnapshot, getProblemSnapshot);

  const engineering = useQuery({
    queryKey: [...WORK_ENGINEERING_SNAPSHOT_KEY, projectName ?? "none", "code-semantic"],
    queryFn: workEngineeringSnapshot,
    enabled: Boolean(projectName),
    staleTime: 5_000,
    refetchOnWindowFocus: false,
  });

  const diff = useQuery({
    queryKey: [...GIT_DIFF_KEY, projectName ?? "none", path, status],
    queryFn: async () => {
      let value = await callCommand<string>("git_file_diff", { path, cached: false });
      if (!value.trim()) value = await callCommand<string>("git_file_diff", { path, cached: true });
      return value;
    },
    enabled: Boolean(projectName) && status !== "clean" && status !== "deleted",
    staleTime: dirty ? 0 : 2_000,
    refetchOnWindowFocus: false,
  });

  return useMemo(() => {
    const snapshot = engineering.data;
    const governance = snapshot?.change_governance;
    const fileGovernance = governance?.files.find((file) => file.path === path) ?? null;
    const fileProblems = problems.diagnostics.filter((problem) => problem.path === path);
    const origin = originFromWorkers(governance?.origin.workers ?? []);

    return {
      path,
      workItemId: snapshot?.work_item_contract.configured ? snapshot.work_item_contract.contract.work_item_id : null,
      scopeState: fileGovernance?.scope_state ?? null,
      reviewState: governance?.review_state ?? null,
      verificationState: governance?.verification.state ?? null,
      gateState: governance?.gate.state ?? null,
      gateReady: governance?.gate.ready ?? false,
      origin: origin.origin,
      originLabel: origin.label,
      problems: fileProblems,
      errors: fileProblems.filter((problem) => problem.severity === "error").length,
      warnings: fileProblems.filter((problem) => problem.severity === "warning").length,
      gitLines: parseGitLineMarkers(diff.data ?? ""),
    };
  }, [diff.data, engineering.data, path, problems.diagnostics]);
}
