import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { basename, pickDirectory } from "../../shared/api/dialog";
import { callCommand, queryKeys } from "../../shared/api/queries";
import { normalizeError } from "../../shared/utils/errors";

export interface ProjectSetupFormState {
  projectName: string;
  projectPath: string;
  projectType: string;
  mainLanguage: string;
}

interface CommandResult {
  ok: boolean;
  command: string;
  stdout: string;
  stderr: string;
  exit_code: number | null;
}

export type ProjectNoticeTone = "ok" | "warn" | "danger";

export interface ProjectSetupNotice {
  tone: ProjectNoticeTone;
  message: string;
}

const INITIAL_PROJECT_SETUP: ProjectSetupFormState = {
  projectName: "",
  projectPath: "",
  projectType: "repository",
  mainLanguage: "",
};

export function useProjectSetup() {
  const queryClient = useQueryClient();
  const [setupForm, setSetupForm] = useState<ProjectSetupFormState>(INITIAL_PROJECT_SETUP);
  const [setupNotice, setSetupNotice] = useState<ProjectSetupNotice | null>(null);
  const [activationNotice, setActivationNotice] = useState<ProjectSetupNotice | null>(null);

  const activateProjectCommand = async (name: string, fallbackMessage = `Could not activate project "${name}".`) => {
    const activated = await callCommand<CommandResult>("project_use", { name });
    if (!activated.ok) {
      throw new Error(activated.stderr || fallbackMessage);
    }
    return name;
  };

  const invalidateActiveProject = (projectName: string) => {
    queryClient.invalidateQueries({ queryKey: ["project_list_configs"] });
    queryClient.invalidateQueries({ queryKey: queryKeys.workspace.snapshot });
    queryClient.invalidateQueries({ queryKey: queryKeys.workspace.activeProject });
    queryClient.invalidateQueries({ queryKey: queryKeys.workflow.state });
    queryClient.invalidateQueries({ queryKey: queryKeys.memory.list(projectName) });
  };

  const addProjectMutation = useMutation({
    mutationFn: async (form: ProjectSetupFormState) => {
      const name = form.projectName.trim();
      const path = form.projectPath.trim();
      if (!name || !path) {
        throw new Error("Project name and path are required.");
      }

      const input = {
        name,
        path,
        project_type: form.projectType.trim() || "repository",
        main_language: form.mainLanguage.trim() || null,
      };

      const added = await callCommand<CommandResult>("project_add", { input });
      const alreadyExists = !added.ok && /already exists/i.test(added.stderr);
      if (!added.ok && !alreadyExists) {
        throw new Error(added.stderr || "Could not add project.");
      }

      await activateProjectCommand(input.name, "Project was added, but could not be activated.");
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
      invalidateActiveProject(projectName);
    },
    onError: (error) => {
      const normalized = normalizeError(error);
      setSetupNotice({ tone: "danger", message: normalized.message });
    },
  });

  const activateProjectMutation = useMutation({
    mutationFn: (name: string) => activateProjectCommand(name),
    onMutate: () => {
      setActivationNotice(null);
    },
    onSuccess: (projectName) => {
      setActivationNotice({ tone: "ok", message: `Project "${projectName}" is now active.` });
      invalidateActiveProject(projectName);
    },
    onError: (error) => {
      const normalized = normalizeError(error);
      setActivationNotice({ tone: "danger", message: normalized.message });
    },
  });

  const browseForProjectPath = async () => {
    const path = await pickDirectory();
    if (!path) return;
    setSetupForm((current) => ({
      ...current,
      projectPath: path,
      projectName: current.projectName.trim() ? current.projectName : basename(path),
    }));
  };

  return {
    setupForm,
    setSetupForm,
    setupNotice,
    activationNotice,
    browseForProjectPath,
    addProject: async () => addProjectMutation.mutateAsync(setupForm),
    isAddingProject: addProjectMutation.isPending,
    activateProject: async (name: string) => activateProjectMutation.mutateAsync(name),
    isActivatingProject: activateProjectMutation.isPending,
    activatingProjectName: activateProjectMutation.variables ?? null,
  };
}
