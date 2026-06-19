import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "../../shared/api/queries";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import * as api from "../../shared/api/orchestrate";

/**
 * Outcome ledger hook (N8-A/B): the learning signal the adaptive router reads.
 * Lists recent step outcomes, the per-(kind, provider) stats for the active
 * project, and the one human mutation — confirm/override a verdict.
 */
export function useOutcomes() {
  const queryClient = useQueryClient();
  const { hasProject, hasTask } = useWorkspace();
  const ready = hasProject && hasTask;

  const stats = useQuery({
    queryKey: queryKeys.outcomes.stats,
    queryFn: () => (ready ? api.outcomesStats() : Promise.resolve([])),
    enabled: ready,
  });

  const list = useQuery({
    queryKey: queryKeys.outcomes.list,
    queryFn: () => (ready ? api.outcomesList(60) : Promise.resolve([])),
    enabled: ready,
  });

  const confirm = useMutation({
    mutationFn: (v: { id: number; verdict: api.Verdict }) => api.outcomesConfirm(v.id, v.verdict),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.outcomes.list });
      queryClient.invalidateQueries({ queryKey: queryKeys.outcomes.stats });
    },
  });

  return {
    ready,
    hasProject,
    hasTask,
    stats: stats.data ?? [],
    statsLoading: stats.isLoading,
    list: list.data ?? [],
    listLoading: list.isLoading,
    confirm,
  };
}
