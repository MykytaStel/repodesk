import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { queryKeys, optionalCommand, callCommand } from "../../shared/api/queries";
import { TokenUsageSnapshot } from "../../shared/types/api";

export function useTokens() {
  const queryClient = useQueryClient();

  const usage = useQuery({
    queryKey: queryKeys.tokens.usage,
    queryFn: () => optionalCommand<TokenUsageSnapshot>("token_usage_snapshot"),
  });

  const estimates = useQuery({
    queryKey: queryKeys.tokens.estimates,
    queryFn: () => optionalCommand<any[]>("get_project_file_token_estimates"),
  });

  const loadEstimates = async () => {
    return await callCommand<any[]>("get_project_file_token_estimates");
  };

  const logTokenUsageMutation = useMutation({
    mutationFn: async (input: any) => await callCommand<TokenUsageSnapshot>("log_token_usage", { input }),
    onSuccess: (data) => {
      queryClient.setQueryData(queryKeys.tokens.usage, data);
    }
  });

  const saveIgnoreRulesMutation = useMutation({
    mutationFn: async (ignoreRules: string[]) => await callCommand<unknown>("save_project_ignore_rules", { ignoreRules }),
    onSuccess: () => queryClient.invalidateQueries()
  });

  return {
    tokens: usage.data,
    fileTokenEstimates: estimates.data ?? [],
    isLoading: usage.isLoading,
    isEstimatesLoading: estimates.isLoading,
    loadEstimates,
    logTokenUsage: logTokenUsageMutation.mutateAsync,
    saveIgnoreRules: saveIgnoreRulesMutation.mutateAsync,
  };
}
