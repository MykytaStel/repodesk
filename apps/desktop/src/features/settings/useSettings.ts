import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { callCommand, optionalCommand, queryKeys } from "../../shared/api/queries";
import type { ProviderPreferences } from "../../shared/api/routing";

export function useSettings() {
  const queryClient = useQueryClient();

  const preferencesQuery = useQuery({
    queryKey: queryKeys.routing.settings,
    queryFn: () => optionalCommand<ProviderPreferences>("provider_preferences"),
  });

  const apiEnvQuery = useQuery({
    queryKey: queryKeys.routing.apiEnv,
    queryFn: () => optionalCommand<any>("get_api_env_diagnostic"),
  });

  const savePreferencesMutation = useMutation({
    mutationFn: (preferences: ProviderPreferences) =>
      callCommand<ProviderPreferences>("save_provider_preferences", { input: preferences }),
    onSuccess: (data) => {
      queryClient.setQueryData(queryKeys.routing.settings, data);
    },
  });

  return {
    providerPreferences: preferencesQuery.data,
    apiEnvDiagnostic: apiEnvQuery.data,
    isLoading: preferencesQuery.isLoading || apiEnvQuery.isLoading,
    savePreferences: async (preferences: ProviderPreferences) => savePreferencesMutation.mutateAsync(preferences),
    isSavingPreferences: savePreferencesMutation.isPending,
  };
}
