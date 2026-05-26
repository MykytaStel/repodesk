import { invoke } from "@tauri-apps/api/core";

export type DesktopAction = {
  id: string;
  title: string;
  description: string;
  category: string;
  risk: string;
  command_preview: string;
};

export type CommandResult = {
  ok: boolean;
  command: string;
  stdout: string;
  stderr: string;
  exit_code: number | null;
};

export type DesktopSnapshot = {
  mode: string;
  workspace_root: string;
  generated_at_ms: number;
  actions: DesktopAction[];
  dashboard: CommandResult;
  workflow: CommandResult;
  doctor: CommandResult;
  security: CommandResult;
  runtime: CommandResult;
  git: CommandResult;
  project_info: CommandResult;
  project_list: CommandResult;
  task_status: CommandResult;
  task_show: CommandResult;
  events: CommandResult;
  knowledge: CommandResult;
};

export type ActionRunResult = {
  id: string;
  title: string;
  risk: string;
  category: string;
  started_at_ms: number;
  finished_at_ms: number;
  result: CommandResult;
};

export type ProjectAddInput = {
  name: string;
  path: string;
  project_type: string;
  main_language?: string | null;
};

export async function loadSnapshot(): Promise<DesktopSnapshot> {
  return invoke<DesktopSnapshot>("desktop_snapshot");
}

export async function loadActions(): Promise<DesktopAction[]> {
  return invoke<DesktopAction[]>("desktop_actions");
}

export async function runAction(actionId: string): Promise<ActionRunResult> {
  return invoke<ActionRunResult>("run_desktop_action", { actionId });
}

export async function loadActionHistory(): Promise<ActionRunResult[]> {
  return invoke<ActionRunResult[]>("action_history");
}

export async function explainAction(actionId: string): Promise<string> {
  return invoke<string>("explain_action", { actionId });
}

export async function projectInfo(): Promise<CommandResult> {
  return invoke<CommandResult>("project_info");
}

export async function projectList(): Promise<CommandResult> {
  return invoke<CommandResult>("project_list");
}

export async function projectUse(name: string): Promise<CommandResult> {
  return invoke<CommandResult>("project_use", { name });
}

export async function projectAdd(input: ProjectAddInput): Promise<CommandResult> {
  return invoke<CommandResult>("project_add", { input });
}

export async function taskNew(title: string): Promise<CommandResult> {
  return invoke<CommandResult>("task_new", { title });
}

export async function taskStatus(): Promise<CommandResult> {
  return invoke<CommandResult>("task_status");
}

export async function taskShow(): Promise<CommandResult> {
  return invoke<CommandResult>("task_show");
}
