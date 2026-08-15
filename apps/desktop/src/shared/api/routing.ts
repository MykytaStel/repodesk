import { invoke } from "@tauri-apps/api/core";

export type ProviderPreferences = {
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
  executor_kind?: string;
  executor_id?: string;
  provider_id?: string | null;
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
  recommended_executor_kind?: string;
  recommended_executor_id?: string;
  recommended_provider_id?: string | null;
  recommended_model?: string | null;
  fallback_executor_id?: string | null;
  fallback_provider_id?: string | null;
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
    executor_kind?: string;
    executor_id?: string;
    provider_id?: string | null;
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

export async function routingDecision(input: RouteRequest): Promise<RouteDecision> {
  return invoke("routing_decision", { input });
}

export async function routingSnapshot(economyMode?: string): Promise<RoutingSnapshot> {
  return invoke("routing_snapshot", { economyMode });
}

export async function providerPreferences(): Promise<ProviderPreferences> {
  return invoke("provider_preferences");
}

export async function saveProviderPreferences(input: ProviderPreferences): Promise<ProviderPreferences> {
  return invoke("save_provider_preferences", { input });
}

export async function saveCodexQuotaStatus(status: string): Promise<ProviderPreferences> {
  return invoke("save_codex_quota_status", { status });
}
