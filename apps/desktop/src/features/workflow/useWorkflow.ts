import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { queryKeys, optionalCommand, callCommand } from "../../shared/api/queries";
import { ActionItem } from "../../shared/types/api";
import { normalizeActions, findNextActionId } from "../../shared/utils/helpers";
import { useWorkspace } from "../../shared/hooks/useWorkspace";

export function useWorkflow() {
  const queryClient = useQueryClient();
  const { hasProject, hasTask } = useWorkspace();

  const state = useQuery({
    queryKey: queryKeys.workflow.state,
    queryFn: () => optionalCommand<unknown>("product_workflow_state"),
  });

  const actionsQuery = useQuery({
    queryKey: queryKeys.workflow.actions,
    queryFn: () => optionalCommand<unknown>("desktop_actions").then(normalizeActions),
  });

  const history = useQuery({
    queryKey: queryKeys.workflow.history,
    queryFn: () => optionalCommand<unknown[]>("action_history").then(res => Array.isArray(res) ? res : []),
  });

  const runActionMutation = useMutation({
    mutationFn: async (actionId: string) => {
      return await callCommand<unknown>("run_desktop_action", { actionId, action_id: actionId });
    },
    onSuccess: () => {
      queryClient.invalidateQueries();
    }
  });

  const doNextSafeStepMutation = useMutation({
    mutationFn: async () => {
      return await callCommand<unknown>("run_next_safe_step");
    },
    onSuccess: () => {
      queryClient.invalidateQueries();
    }
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
  };
}
