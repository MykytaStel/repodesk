import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { queryKeys, optionalCommand, callCommand } from "../../shared/api/queries";
import { invalidateQueryDomains } from "../../shared/api/cacheInvalidation";
import { normalizeActions, findNextActionId } from "../../shared/utils/helpers";
import { useWorkspace } from "../../shared/hooks/useWorkspace";

export function useWorkflow() {
  const queryClient = useQueryClient();
  const { hasProject, hasTask } = useWorkspace();

  const state = useQuery({
    queryKey: queryKeys.workflow.state,
    queryFn: () => optionalCommand<unknown>("product_workflow_state"),
    staleTime: 2_000,
  });

  const actionsQuery = useQuery({
    queryKey: queryKeys.workflow.actions,
    queryFn: () => optionalCommand<unknown>("desktop_actions").then(normalizeActions),
    staleTime: 10_000,
  });

  const history = useQuery({
    queryKey: queryKeys.workflow.history,
    queryFn: () => optionalCommand<unknown[]>("action_history").then((result) => Array.isArray(result) ? result : []),
    staleTime: 5_000,
  });

  const refreshWorkflowDomains = () =>
    invalidateQueryDomains(queryClient, ["workspace", "work", "git", "code", "runs"]);

  const runActionMutation = useMutation({
    mutationFn: async (actionId: string) => {
      return await callCommand<unknown>("run_desktop_action", { actionId, action_id: actionId });
    },
    onSuccess: refreshWorkflowDomains,
  });

  const doNextSafeStepMutation = useMutation({
    mutationFn: async () => {
      return await callCommand<unknown>("run_next_safe_step");
    },
    onSuccess: refreshWorkflowDomains,
  });

  const commitMutation = useMutation({
    mutationFn: async (message: string) => {
      return await callCommand<{ ok: boolean; stdout: string; stderr: string }>("commit_ready_changes", { message });
    },
    onSuccess: refreshWorkflowDomains,
  });

  const workflow = state.data;
  const actions = actionsQuery.data ?? [];
  const nextActionId = findNextActionId(workflow, actions, hasProject, hasTask);
  const nextAction = actions.find((action) => action.id === nextActionId) ?? null;

  return {
    workflow,
    actions,
    history: history.data ?? [],
    nextAction,
    isLoading: state.isLoading || actionsQuery.isLoading || history.isLoading,
    runAction: runActionMutation.mutateAsync,
    doNextSafeStep: doNextSafeStepMutation.mutateAsync,
    isRunning: runActionMutation.isPending || doNextSafeStepMutation.isPending,
    commitChanges: commitMutation.mutateAsync,
    isCommitting: commitMutation.isPending,
    commitError: commitMutation.error as Error | null,
  };
}
