export type TabId = "dashboard" | "workflow" | "tokens" | "models" | "code" | "git" | "memory" | "orchestrate" | "settings" | "system" | "debug";
export type DebugStatus = "success" | "error";
export type ToastKind = "success" | "error" | "warning" | "info";
export type Theme = "dark" | "light" | "system" | "midnight" | "nord" | "high-contrast";
export type UnknownRecord = Record<string, unknown>;

export interface DebugEvent {
  id: number;
  command: string;
  args?: UnknownRecord;
  status: DebugStatus;
  durationMs: number;
  timestamp: string;
  preview?: string;
  error?: string;
}

export interface ToastMessage {
  id: number;
  kind: ToastKind;
  title: string;
  message?: string;
}

export interface ActionItem {
  id: string;
  label: string;
  title: string;
  description: string;
  risk: string;
  category: string;
}

export interface ProviderSettings {
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

export interface RouteCandidate {
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
}

export interface RouteDecision {
  task_kind: string;
  recommended_provider: string;
  recommended_model?: string | null;
  fallback_provider?: string | null;
  fallback_model?: string | null;
  score: number;
  decision_level: string;
  blockers: string[];
  warnings: string[];
  required_guardrails: string[];
  candidates: RouteCandidate[];
  estimated_total_tokens: number;
}

export interface RoutingSnapshot {
  generated_at_ms: number;
  request: {
    task_kind: string;
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
}

export interface TokenUsageSnapshot {
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
  active_artifacts: TokenArtifactEstimate[];
  cost_estimate: {
    estimated_total_units: number;
    currency_label: string;
    note: string;
  };
}

export interface ApiEnvDiagnostic {
  openai_api_key_set: boolean;
  gemini_api_key_set: boolean;
  anthropic_api_key_set: boolean;
}

export interface TokenUsageItem {
  provider: string;
  model?: string | null;
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
  estimated_cost_units?: number | null;
  currency_label?: string | null;
}

export interface TokenArtifactEstimate {
  kind: string;
  title: string;
  path?: string | null;
  exists: boolean;
  size_bytes: number;
  estimated_tokens?: number | null;
  status: string;
  recommendation: string;
  error?: string | null;
}

export interface ModelHealthSnapshot {
  generated_at_ms: number;
  providers: ProviderHealth[];
  warnings: string[];
}

export interface ProviderHealth {
  id: string;
  label: string;
  enabled: boolean;
  auth_status: string;
  reachability: "working" | "auth_missing" | "unreachable" | "rate_limited" | "disabled" | string;
  models: ModelStatus[];
  error_summary?: string | null;
}

export interface ModelStatus {
  id: string;
  provider: string;
  available: boolean;
  loaded?: boolean | null;
  context_window?: number | null;
  notes?: string | null;
}

export interface AgentConfig {
  name: string;
  kind: string;
  role: string;
  default_budget_tokens: number;
  allowed_actions: string[];
  forbidden_actions: string[];
  preferred_for: string[];
}

export interface AgentsConfig {
  agents: AgentConfig[];
}

export interface Capability {
  name: string;
  kind: string;
  enabled: boolean;
  local: boolean;
  risk: string;
  boundary: string;
  preferred_for: string[];
  allowed_actions: string[];
  forbidden_actions: string[];
}

export interface CapabilitiesConfig {
  capabilities: Capability[];
}

export interface PeripheralConfig {
  name: string;
  kind: string;
  access: string;
  risk: string;
  allowed_actions: string[];
  forbidden_actions: string[];
}

export interface PeripheralsConfig {
  peripherals: PeripheralConfig[];
}

export interface BrainModule {
  name: string;
  layer: string;
  status: string;
  purpose: string;
}

export interface SetupFormState {
  projectName: string;
  projectPath: string;
  projectType: string;
  mainLanguage: string;
  taskTitle: string;
}

export interface TokenLogFormState {
  provider: string;
  model: string;
  inputTokens: string;
  outputTokens: string;
  category: string;
  notes: string;
}

