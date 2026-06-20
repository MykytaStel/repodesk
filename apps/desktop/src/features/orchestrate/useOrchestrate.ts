import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "../../shared/api/queries";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import * as api from "../../shared/api/orchestrate";

/**
 * Orchestrator hook: build a plan (mutation), run it (dry-run or real, mutation),
 * and read the latest run (query). A successful real run captures Memory Brain
 * proposals, so it invalidates the memory proposal views too.
 */
export function useOrchestrate() {
  const queryClient = useQueryClient();
  const { projectName, hasProject, hasTask } = useWorkspace();
  const ready = hasProject && hasTask;

  const status = useQuery({
    queryKey: queryKeys.orchestrate.status,
    queryFn: () => (ready ? api.orchestrateStatus() : Promise.resolve(null)),
    enabled: ready,
  });

  const runs = useQuery({
    queryKey: queryKeys.orchestrate.runs,
    queryFn: () => (ready ? api.orchestrationRuns() : Promise.resolve([])),
    enabled: ready,
  });

  const timeline = useQuery({
    queryKey: queryKeys.orchestrate.timeline,
    queryFn: () => (ready ? api.taskTimeline() : Promise.resolve([])),
    enabled: ready,
  });

  const executors = useQuery({
    queryKey: queryKeys.orchestrate.executors,
    queryFn: () => (ready ? api.codingAgentExecutors() : Promise.resolve([])),
    enabled: ready,
  });

  const plan = useMutation({
    mutationFn: (v: { goal: string; overrideProvider?: string; overrideModel?: string }) =>
      api.orchestratePlan(v.goal || undefined, v.overrideProvider, v.overrideModel),
  });

  const run = useMutation({
    mutationFn: (v: {
      goal: string;
      dryRun: boolean;
      maxCost?: number | null;
      approveCodingAgents: boolean;
      overrideProvider?: string;
      overrideModel?: string;
    }) =>
      api.orchestrateRun(
        v.goal || undefined,
        v.dryRun,
        v.maxCost ?? null,
        v.approveCodingAgents,
        v.overrideProvider,
        v.overrideModel,
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.orchestrate.status });
      queryClient.invalidateQueries({ queryKey: queryKeys.orchestrate.runs });
      queryClient.invalidateQueries({ queryKey: queryKeys.orchestrate.timeline });
      // A real run produced memory capture proposals — refresh the review queue.
      queryClient.invalidateQueries({ queryKey: queryKeys.memory.proposals(projectName) });
    },
  });

  // Load a specific past run's full detail (for the history list → RunPanel).
  const showRun = useMutation({
    mutationFn: (runId: string) => api.orchestrateShow(runId),
  });

  // Review a coding-agent changeset. Accept can stage/apply files; reject may
  // discard in-place changes or leave an isolated worktree untouched.
  const review = useMutation({
    mutationFn: (v: { runId: string; action: api.ReviewAction }) =>
      api.orchestrateReview(v.runId, v.action),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.git.snapshot });
    },
  });

  // Autonomous loop (N8-C): plan → run → re-plan/retry under guardrails. A real
  // loop records outcomes and may capture memory proposals, so refresh both.
  const loop = useMutation({
    mutationFn: (v: {
      goal: string;
      maxIterations?: number;
      maxCost?: number | null;
      dryRun: boolean;
      approvePaid: boolean;
      approveCodingAgents: boolean;
      overrideProvider?: string;
      overrideModel?: string;
    }) =>
      api.orchestrateLoop({
        goal: v.goal || undefined,
        maxIterations: v.maxIterations,
        maxCost: v.maxCost ?? null,
        dryRun: v.dryRun,
        approvePaid: v.approvePaid,
        approveCodingAgents: v.approveCodingAgents,
        overrideProvider: v.overrideProvider,
        overrideModel: v.overrideModel,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.orchestrate.status });
      queryClient.invalidateQueries({ queryKey: queryKeys.orchestrate.runs });
      queryClient.invalidateQueries({ queryKey: queryKeys.orchestrate.timeline });
      queryClient.invalidateQueries({ queryKey: queryKeys.outcomes.list });
      queryClient.invalidateQueries({ queryKey: queryKeys.outcomes.stats });
      queryClient.invalidateQueries({ queryKey: queryKeys.memory.proposals(projectName) });
    },
  });

  return {
    projectName,
    hasProject,
    hasTask,
    ready,
    status: status.data ?? null,
    statusLoading: status.isLoading,
    runs: runs.data ?? [],
    timeline: timeline.data ?? [],
    executors: executors.data ?? [],
    executorsLoading: executors.isLoading,
    plan,
    run,
    showRun,
    review,
    loop,
  };
}
