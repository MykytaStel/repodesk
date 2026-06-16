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

  const plan = useMutation({
    mutationFn: (goal: string) => api.orchestratePlan(goal || undefined),
  });

  const run = useMutation({
    mutationFn: (v: { goal: string; dryRun: boolean; maxCost?: number | null }) =>
      api.orchestrateRun(v.goal || undefined, v.dryRun, v.maxCost ?? null),
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

  return {
    projectName,
    hasProject,
    hasTask,
    ready,
    status: status.data ?? null,
    statusLoading: status.isLoading,
    runs: runs.data ?? [],
    timeline: timeline.data ?? [],
    plan,
    run,
    showRun,
  };
}
