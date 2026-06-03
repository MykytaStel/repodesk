import { invoke } from "@tauri-apps/api/core";

export type DbStatus = {
  path: string;
  exists: boolean;
  ok: boolean;
  tables: string[];
  error?: string | null;
};

export type ApiEnvDiagnostic = {
  openai_api_key_set: boolean;
  gemini_api_key_set: boolean;
  anthropic_api_key_set: boolean;
};

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

export async function dbStatus(): Promise<DbStatus> {
  return invoke("db_status");
}

export async function getApiEnvDiagnostic(): Promise<ApiEnvDiagnostic> {
  return invoke("get_api_env_diagnostic");
}

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
