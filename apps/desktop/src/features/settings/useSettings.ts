import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { callCommand, optionalCommand, queryKeys } from "../../shared/api/queries";

interface ProviderSettings {
  ollama_enabled: boolean;
  ollama_url: string;
  ollama_model: string;
  lm_studio_enabled: boolean;
  lm_studio_url: string;
  llamafile_enabled: boolean;
  llamafile_url: string;
  localai_enabled: boolean;
  localai_url: string;
  chatgpt_enabled: boolean;
  codex_enabled: boolean;
  gemini_enabled: boolean;
  openai_api_enabled: boolean;
  openai_api_key_env_var: string;
  gemini_api_enabled: boolean;
  gemini_api_key_env_var: string;
  anthropic_api_enabled: boolean;
  anthropic_api_key: string;
  openai_api_key: string;
  gemini_api_key: string;
  allow_paid_agents: boolean;
  codex_quota_status: string;
  preferred_patch_provider: string;
  preferred_compression_provider: string;
  preferred_review_provider: string;
  notes: string;
}

export function useSettings() {
  const queryClient = useQueryClient();

  const settingsQuery = useQuery({
    queryKey: queryKeys.routing.settings,
    queryFn: () => optionalCommand<ProviderSettings>("provider_settings"),
  });

  const apiEnvQuery = useQuery({
    queryKey: queryKeys.routing.apiEnv,
    queryFn: () => optionalCommand<any>("get_api_env_diagnostic"),
  });

  const saveSettingsMutation = useMutation({
    mutationFn: (settings: ProviderSettings) =>
      callCommand<ProviderSettings>("save_provider_settings", { input: settings }),
    onSuccess: (data) => {
      queryClient.setQueryData(queryKeys.routing.settings, data);
    },
  });

  return {
    providerSettings: settingsQuery.data,
    apiEnvDiagnostic: apiEnvQuery.data,
    isLoading: settingsQuery.isLoading || apiEnvQuery.isLoading,
    saveSettings: async (settings: ProviderSettings) => saveSettingsMutation.mutateAsync(settings),
    isSavingSettings: saveSettingsMutation.isPending,
  };
}
