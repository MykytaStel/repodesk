import { invoke } from "@tauri-apps/api/core";
import type { ProjectAddInput } from "./project";

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

export type ActionRunResult = {
  id: string;
  title: string;
  risk: string;
  category: string;
  started_at_ms: number;
  finished_at_ms: number;
  result: CommandResult;
};

export type WorkflowStep = {
  id: string;
  title: string;
  description: string;
  status: "done" | "current" | "blocked" | string;
  action_id?: string | null;
  artifact_kind?: string | null;
  command_preview?: string | null;
  blocker?: string | null;
};

export type ArtifactStatus = {
  kind: string;
  title: string;
  path?: string | null;
  exists: boolean;
  size_bytes: number;
};

export type ArtifactContent = {
  kind: string;
  title: string;
  path: string;
  exists: boolean;
  content: string;
  size_bytes: number;
};

export type CommitReadiness = {
  status: "ready" | "blocked" | "warning" | "nothing_to_commit" | "not_a_repo";
  headline: string;
  blockers: string[];
  warnings: string[];
  is_dirty: boolean;
  changed_count: number;
  branch?: string | null;
};

export type ProductWorkflowState = {
  generated_at_ms: number;
  overall_status: string;
  primary_cta: string;
  recommended_action_id?: string | null;
  recommended_action_title?: string | null;
  steps: WorkflowStep[];
  artifacts: ArtifactStatus[];
  project_ok: boolean;
  task_ok: boolean;
  context_ok: boolean;
  smart_context_ok: boolean;
  prompts_ok: boolean;
  checks_ok: boolean;
  safety_ok: boolean;
  commit_readiness: CommitReadiness;
  project_info: CommandResult;
  task_status: CommandResult;
  workflow_hint: CommandResult;
  security_verdict: CommandResult;
  checks_summary_preview?: string | null;
};

export type DesktopSnapshot = {
  mode: string;
  workspace_root: string;
  generated_at_ms: number;
  actions: DesktopAction[];
  workflow_state: ProductWorkflowState;
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

export async function getSnapshot(): Promise<DesktopSnapshot> {
  return invoke("desktop_snapshot");
}

export async function getWorkflowState(): Promise<ProductWorkflowState> {
  return invoke("product_workflow_state");
}

export async function readArtifact(kind: string): Promise<ArtifactContent> {
  return invoke("read_artifact", { kind });
}

export async function getActions(): Promise<DesktopAction[]> {
  return invoke("desktop_actions");
}

export async function explainAction(actionId: string): Promise<string> {
  return invoke("explain_action", { actionId });
}

export async function runDesktopAction(actionId: string): Promise<ActionRunResult> {
  return invoke("run_desktop_action", { actionId });
}

export async function runNextSafeStep(): Promise<ActionRunResult> {
  return invoke("run_next_safe_step");
}

export async function getActionHistory(): Promise<ActionRunResult[]> {
  return invoke("action_history");
}

export async function projectInfo(): Promise<CommandResult> {
  return invoke("project_info");
}

export async function projectList(): Promise<CommandResult> {
  return invoke("project_list");
}

export async function projectUse(name: string): Promise<CommandResult> {
  return invoke("project_use", { name });
}

export async function projectAdd(input: ProjectAddInput): Promise<CommandResult> {
  return invoke("project_add", { input });
}

export async function taskNew(title: string): Promise<CommandResult> {
  return invoke("task_new", { title });
}

export async function taskStatus(): Promise<CommandResult> {
  return invoke("task_status");
}

export async function taskShow(): Promise<CommandResult> {
  return invoke("task_show");
}
