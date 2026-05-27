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
};

export type TaskKind = "compress" | "summarize" | "plan" | "review" | "patch" | "debug" | "checks" | "manual";

export type RouteRequest = {
  task_kind: TaskKind;
  estimated_input_tokens: number;
  estimated_output_tokens: number;
  risk_level: string;
  changed_file_count: number;
  requires_write: boolean;
  context_safe?: boolean | null;
  checks_ok?: boolean | null;
  guard_allowed?: boolean | null;
  git_dirty?: boolean | null;
  max_cost_units?: number | null;
};

export type RouteCandidate = {
  provider: string;
  label: string;
  kind: string;
  model?: string | null;
  score: number;
  blocked: boolean;
  blockers: string[];
  warnings: string[];
  required_guardrails: string[];
  estimated_cost_units: number;
};

export type RouteDecision = {
  task_kind: TaskKind;
  recommended_provider: string;
  recommended_model?: string | null;
  fallback_provider?: string | null;
  fallback_model?: string | null;
  score: number;
  decision_level: "allow" | "warn" | "block" | string;
  blockers: string[];
  warnings: string[];
  required_guardrails: string[];
  candidates: RouteCandidate[];
  estimated_total_tokens: number;
};

export type RoutingSnapshot = {
  generated_at_ms: number;
  request: RouteRequest;
  decision: RouteDecision;
  capacities: Array<{
    provider: string;
    label: string;
    kind: string;
    enabled: boolean;
    auth_status: string;
    reachability: string;
    models: string[];
    preferred_model?: string | null;
    daily_remaining_tokens: number;
    estimated_cost_units: number;
    quota_status: string;
    paid_agents_allowed: boolean;
    max_patch_files: number;
  }>;
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
    today_total_tokens: number;
    remaining_daily_tokens: number;
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

export async function routingDecision(input: RouteRequest): Promise<RouteDecision> {
  return invoke("routing_decision", { input });
}

export async function routingSnapshot(economyMode?: string): Promise<RoutingSnapshot> {
  return invoke("routing_snapshot", { economyMode });
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

export async function saveCodexQuotaStatus(status: string): Promise<ProviderSettings> {
  return invoke("save_codex_quota_status", { status });
}

export type FileTokenEstimate = {
  path: string;
  bytes: number;
  estimated_tokens: number;
  status: string;
};

export type ProjectConfig = {
  name: string;
  path: string;
  project_type: string;
  main_language?: string | null;
  checks: string[];
  context_ignore: string[];
  created_at: string;
  updated_at: string;
};

export async function getActiveProjectConfig(): Promise<ProjectConfig> {
  return invoke("get_active_project_config");
}

export async function saveProjectIgnoreRules(ignoreRules: string[]): Promise<void> {
  return invoke("save_project_ignore_rules", { ignoreRules });
}

export async function getProjectFileTokenEstimates(): Promise<FileTokenEstimate[]> {
  return invoke("get_project_file_token_estimates");
}

export type MemoryEntry = {
  id: number;
  timestamp: string;
  project: string;
  content: string;
  category: string;
  tags: string[];
};

export async function readProjectMemory(project: string): Promise<MemoryEntry[]> {
  return invoke("memory_list", { project });
}

export async function appendProjectMemory(project: string, content: string, category: string = "general", tags: string[] = []): Promise<MemoryEntry> {
  return invoke("memory_add", { project, content, category, tags });
}

export type ApiEnvDiagnostic = {
  openai_api_key_set: boolean;
  gemini_api_key_set: boolean;
  anthropic_api_key_set: boolean;
};

export async function getApiEnvDiagnostic(): Promise<ApiEnvDiagnostic> {
  return invoke("get_api_env_diagnostic");
}

export type AgentConfig = {
  name: string;
  kind: string;
  role: string;
  default_budget_tokens: number;
  allowed_actions: string[];
  forbidden_actions: string[];
  preferred_for: string[];
};

export type AgentsConfig = {
  agents: AgentConfig[];
};

export type Capability = {
  name: string;
  kind: string;
  enabled: boolean;
  local: boolean;
  risk: string;
  boundary: string;
  preferred_for: string[];
  allowed_actions: string[];
  forbidden_actions: string[];
};

export type CapabilitiesConfig = {
  capabilities: Capability[];
};

export type PeripheralConfig = {
  name: string;
  kind: string;
  access: string;
  risk: string;
  allowed_actions: string[];
  forbidden_actions: string[];
};

export type PeripheralsConfig = {
  peripherals: PeripheralConfig[];
};

export type BrainModule = {
  name: string;
  layer: string;
  status: string;
  purpose: string;
};

export async function getSystemAgents(): Promise<AgentsConfig> {
  return invoke("get_system_agents");
}

export async function getSystemCapabilities(): Promise<CapabilitiesConfig> {
  return invoke("get_system_capabilities");
}

export async function getSystemPeripherals(): Promise<PeripheralsConfig> {
  return invoke("get_system_peripherals");
}

export async function getSystemModules(): Promise<BrainModule[]> {
  return invoke("get_system_modules");
}
