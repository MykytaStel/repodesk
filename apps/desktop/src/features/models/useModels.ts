import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { queryKeys, optionalCommand, callCommand } from "../../shared/api/queries";
import { ModelHealthSnapshot } from "../../shared/types/api";

export function useModels() {
  const queryClient = useQueryClient();

  const { data: models, isLoading } = useQuery({
    queryKey: queryKeys.models.health,
    queryFn: () => optionalCommand<ModelHealthSnapshot>("model_health_snapshot"),
  });

  const refreshMutation = useMutation({
    mutationFn: async () => await callCommand<ModelHealthSnapshot>("refresh_model_health"),
    onSuccess: (data) => {
      queryClient.setQueryData(queryKeys.models.health, data);
    }
  });

  const workingProviders = models?.providers.filter((provider) => provider.reachability === "working").length ?? 0;
  const modelCount = models?.providers.reduce((total, provider) => total + provider.models.length, 0) ?? 0;

  return { 
    models, 
    workingProviders, 
    modelCount, 
    isLoading,
    refreshModels: refreshMutation.mutateAsync,
    isRefreshing: refreshMutation.isPending
  };
}
