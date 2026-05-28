import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { queryKeys, callCommand, optionalCommand } from "../../shared/api/queries";
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
  allow_paid_agents: boolean;
  codex_quota_status: string;
  preferred_patch_provider: string;
  preferred_compression_provider: string;
  preferred_review_provider: string;
  notes: string;
}

interface SetupFormState {
  projectName: string;
  projectPath: string;
  projectType: string;
  mainLanguage: string;
  taskTitle: string;
}

export function useSettings() {
  const queryClient = useQueryClient();
  const { projectName } = useWorkspace();
  
  const [setupForm, setSetupForm] = useState<SetupFormState>({
    projectName: "repodesk",
    projectPath: "",
    projectType: "rust-tauri",
    mainLanguage: "rust",
    taskTitle: "Improve RepoDesk workflow",
  });
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
    mutationFn: (settings: ProviderSettings) => callCommand<ProviderSettings>("save_provider_settings", { input: settings }),
    onSuccess: (data) => {
      queryClient.setQueryData(queryKeys.routing.settings, data);
    },
  });

  const appendMemoryMutation = useMutation({
    mutationFn: async (content: string) => {
      await callCommand("memory_add", {
        project: projectName,
        content: content,
        category: "general",
        tags: []
      });
    },
    onSuccess: () => {
      setMemoryAppendInput("");
      queryClient.invalidateQueries({ queryKey: queryKeys.memory.list(projectName) });
    },
  });

  const addProjectMutation = useMutation({
    mutationFn: async (form: SetupFormState) => {
      const input = {
        name: form.projectName.trim(),
        path: form.projectPath.trim(),
        project_type: form.projectType.trim(),
        main_language: form.mainLanguage.trim() || null,
      };
      const added = await callCommand<{ ok: boolean }>("project_add", { input });
      if (added.ok) {
        await callCommand("project_use", { name: input.name });
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.workspace.snapshot });
    },
  });

  const createTaskMutation = useMutation({
    mutationFn: async (title: string) => {
      await callCommand("task_new", { title: title.trim() });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.workspace.snapshot });
      queryClient.invalidateQueries({ queryKey: queryKeys.workflow.state });
    },
  });

  return {
    providerSettings: settingsQuery.data,
    apiEnvDiagnostic: apiEnvQuery.data,
    projectMemory: memoryQuery.data || [],
    isLoading: settingsQuery.isLoading || apiEnvQuery.isLoading || memoryQuery.isLoading,
    
    setupForm,
    setSetupForm,
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

    addProjectFromSetup: async () => addProjectMutation.mutateAsync(setupForm),
    isAddingProject: addProjectMutation.isPending,

    createTaskFromSetup: async () => createTaskMutation.mutateAsync(setupForm.taskTitle),
    isCreatingTask: createTaskMutation.isPending,
  };
}
