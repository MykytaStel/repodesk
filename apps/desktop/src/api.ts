import { invoke } from "@tauri-apps/api/core";

export type DesktopAction = {
  id: string;
  title: string;
  description: string;
  category: string;
  risk: string;
  command_preview: string;
};


export type DbStatus = {
  path: string;
  exists: boolean;
  ok: boolean;
  tables: string[];
  error?: string | null;
};

export type ProviderSettings = {
  ollama_enabled: boolean;
  ollama_url: string;
  ollama_model: string;
  lm_studio_enabled: boolean;
  lm_studio_url: string;
  chatgpt_enabled: boolean;
  codex_enabled: boolean;
  gemini_enabled: boolean;
  openai_api_enabled: boolean;
  openai_api_key_env_var: string;
  gemini_api_enabled: boolean;
  gemini_api_key_env_var: string;
  allow_paid_agents: boolean;
  preferred_patch_provider: string;
  preferred_compression_provider: string;
  preferred_review_provider: string;
  notes: string;
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

export type ProjectAddInput = {
  name: string;
  path: string;
  project_type: string;
  main_language?: string | null;
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

export type TokenUsageItem = {
  provider: string;
  model?: string | null;
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
  estimated_cost_units?: number | null;
  currency_label?: string | null;
};

export type TokenUsageSnapshot = {
  generated_at_ms: number;
  totals: {
    entries_count: number;
    total_input_tokens: number;
    total_output_tokens: number;
    total_tokens: number;
  };
  by_provider: TokenUsageItem[];
  by_model: TokenUsageItem[];
  active_artifacts: Array<{
    kind: string;
    title: string;
    path?: string | null;
    exists: boolean;
    size_bytes: number;
    estimated_tokens?: number | null;
    status: string;
    recommendation: string;
    error?: string | null;
  }>;
  cost_estimate: {
    estimated_total_units: number;
    currency_label: string;
    note: string;
  };
};

export type LogTokenUsageInput = {
  provider: string;
  model?: string | null;
  input_tokens: number;
  output_tokens: number;
  category: string;
  notes?: string | null;
};

export type ModelStatus = {
  id: string;
  provider: string;
  available: boolean;
  loaded?: boolean | null;
  context_window?: number | null;
  notes?: string | null;
};

export type ProviderHealth = {
  id: string;
  label: string;
  enabled: boolean;
  auth_status: string;
  reachability: string;
  models: ModelStatus[];
  error_summary?: string | null;
};

export type ModelHealthSnapshot = {
  generated_at_ms: number;
  providers: ProviderHealth[];
  warnings: string[];
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

export async function tokenUsageSnapshot(): Promise<TokenUsageSnapshot> {
  return invoke("token_usage_snapshot");
}

export async function logTokenUsage(input: LogTokenUsageInput): Promise<TokenUsageSnapshot> {
  return invoke("log_token_usage", { input });
}

export async function modelHealthSnapshot(): Promise<ModelHealthSnapshot> {
  return invoke("model_health_snapshot");
}

export async function refreshModelHealth(): Promise<ModelHealthSnapshot> {
  return invoke("refresh_model_health");
}


export async function dbStatus(): Promise<DbStatus> {
  return invoke("db_status");
}

export async function providerSettings(): Promise<ProviderSettings> {
  return invoke("provider_settings");
}

export async function saveProviderSettings(input: ProviderSettings): Promise<ProviderSettings> {
  return invoke("save_provider_settings", { input });
}
