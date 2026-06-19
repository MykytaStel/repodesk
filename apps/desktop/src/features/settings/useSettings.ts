import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { queryKeys, callCommand, optionalCommand } from "../../shared/api/queries";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { normalizeError } from "../../shared/utils/errors";

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

interface SetupFormState {
  projectName: string;
  projectPath: string;
  projectType: string;
  mainLanguage: string;
  taskTitle: string;
}

interface CommandResult {
  ok: boolean;
  command: string;
  stdout: string;
  stderr: string;
  exit_code: number | null;
}

interface SetupNotice {
  tone: "ok" | "warn" | "danger";
  message: string;
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
  const [setupNotice, setSetupNotice] = useState<SetupNotice | null>(null);
  const [taskNotice, setTaskNotice] = useState<SetupNotice | null>(null);

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
      const name = form.projectName.trim();
      const path = form.projectPath.trim();
      if (!name || !path) {
        throw new Error("Project name and path are required.");
      }

      const input = {
        name,
        path,
        project_type: form.projectType.trim(),
        main_language: form.mainLanguage.trim() || null,
      };

      const added = await callCommand<CommandResult>("project_add", { input });
      const alreadyExists = !added.ok && /already exists/i.test(added.stderr);
      if (!added.ok && !alreadyExists) {
        throw new Error(added.stderr || "Could not add project.");
      }

      const activated = await callCommand<CommandResult>("project_use", { name: input.name });
      if (!activated.ok) {
        throw new Error(activated.stderr || "Project was added, but could not be activated.");
      }

      return { projectName: input.name, alreadyExists };
    },
    onMutate: () => {
      setSetupNotice({ tone: "warn", message: "Adding project and switching workspace..." });
    },
    onSuccess: ({ projectName, alreadyExists }) => {
      setSetupNotice({
        tone: "ok",
        message: alreadyExists
          ? `Project "${projectName}" already existed, so RepoDesk activated it.`
          : `Project "${projectName}" was added and activated.`,
      });
      queryClient.invalidateQueries({ queryKey: queryKeys.workspace.snapshot });
      queryClient.invalidateQueries({ queryKey: queryKeys.workspace.activeProject });
      queryClient.invalidateQueries({ queryKey: queryKeys.workflow.state });
      queryClient.invalidateQueries({ queryKey: queryKeys.memory.list(projectName) });
    },
    onError: (error) => {
      const normalized = normalizeError(error);
      setSetupNotice({ tone: "danger", message: normalized.message });
    },
  });

  const createTaskMutation = useMutation({
    mutationFn: async (title: string) => {
      const taskTitle = title.trim();
      if (!taskTitle) {
        throw new Error("Task title is required.");
      }

      const created = await callCommand<CommandResult>("task_new", { title: taskTitle });
      if (!created.ok) {
        throw new Error(created.stderr || "Could not create task.");
      }

      return taskTitle;
    },
    onMutate: () => {
      setTaskNotice({ tone: "warn", message: "Creating active task..." });
    },
    onSuccess: (title) => {
      setTaskNotice({ tone: "ok", message: `Task "${title}" was created and activated.` });
      queryClient.invalidateQueries({ queryKey: queryKeys.workspace.snapshot });
      queryClient.invalidateQueries({ queryKey: queryKeys.workflow.state });
    },
    onError: (error) => {
      const normalized = normalizeError(error);
      setTaskNotice({ tone: "danger", message: normalized.message });
    },
  });

  return {
    providerSettings: settingsQuery.data,
    apiEnvDiagnostic: apiEnvQuery.data,
    projectMemory: memoryQuery.data || [],
    isLoading: settingsQuery.isLoading || apiEnvQuery.isLoading || memoryQuery.isLoading,
    
    setupForm,
    setSetupForm,
    setupNotice,
    taskNotice,
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
