import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { queryKeys, optionalCommand, callCommand } from "../../shared/api/queries";
import { TokenUsageSnapshot } from "../../shared/types/api";

/** One day's aggregated usage on the cost trend (mirrors core CostTrendPoint). */
export type CostTrendPoint = {
  date: string;
  total_tokens: number;
  cost_units: number;
};

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

  const costTrend = useQuery({
    queryKey: queryKeys.tokens.costTrend,
    queryFn: () => optionalCommand<CostTrendPoint[]>("token_cost_trend", { days: 14 }),
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
    costTrend: costTrend.data ?? [],
    isLoading: usage.isLoading,
    isEstimatesLoading: estimates.isLoading,
    loadEstimates,
    logTokenUsage: logTokenUsageMutation.mutateAsync,
    saveIgnoreRules: saveIgnoreRulesMutation.mutateAsync,
  };
}
