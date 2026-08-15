import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { callCommand, optionalCommand, queryKeys } from "../../shared/api/queries";
import { useWorkspace } from "../../shared/hooks/useWorkspace";

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
  const { projectName } = useWorkspace();
  const [memoryAppendInput, setMemoryAppendInput] = useState("");

  const settingsQuery = useQuery({
    queryKey: queryKeys.routing.settings,
    queryFn: () => optionalCommand<ProviderSettings>("provider_settings"),
  });

  const apiEnvQuery = useQuery({
    queryKey: queryKeys.routing.apiEnv,
    queryFn: () => optionalCommand<any>("get_api_env_diagnostic"),
  });

  const memoryQuery = useQuery({
    queryKey: queryKeys.memory.list(projectName),
    queryFn: () => optionalCommand<any[]>("memory_list", { project: projectName }),
    enabled: projectName !== "No active project" && projectName !== "-",
  });

  const saveSettingsMutation = useMutation({
    mutationFn: (settings: ProviderSettings) =>
      callCommand<ProviderSettings>("save_provider_settings", { input: settings }),
    onSuccess: (data) => {
      queryClient.setQueryData(queryKeys.routing.settings, data);
    },
  });

  const appendMemoryMutation = useMutation({
    mutationFn: async (content: string) => {
      await callCommand("memory_add", {
        project: projectName,
        content,
        category: "general",
        tags: [],
      });
    },
    onSuccess: () => {
      setMemoryAppendInput("");
      queryClient.invalidateQueries({ queryKey: queryKeys.memory.list(projectName) });
    },
  });

  return {
    providerSettings: settingsQuery.data,
    apiEnvDiagnostic: apiEnvQuery.data,
    projectMemory: memoryQuery.data || [],
    isLoading: settingsQuery.isLoading || apiEnvQuery.isLoading || memoryQuery.isLoading,

    memoryAppendInput,
    setMemoryAppendInput,

    saveSettings: async (settings: ProviderSettings) => saveSettingsMutation.mutateAsync(settings),
    isSavingSettings: saveSettingsMutation.isPending,

    handleAppendMemory: async () => {
      if (memoryAppendInput.trim()) {
        await appendMemoryMutation.mutateAsync(memoryAppendInput.trim());
      }
    },
    isAppendingMemory: appendMemoryMutation.isPending,
  };
}
