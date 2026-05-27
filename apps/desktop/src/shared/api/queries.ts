import { invoke } from "@tauri-apps/api/core";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";

type UnknownRecord = Record<string, unknown>;

export const queryKeys = {
  workspace: {
    snapshot: ["desktop_snapshot"] as const,
    dbStatus: ["db_status"] as const,
    activeProject: ["get_active_project_config"] as const,
  },
  workflow: {
    state: ["product_workflow_state"] as const,
    actions: ["desktop_actions"] as const,
    history: ["action_history"] as const,
  },
  git: {
    snapshot: ["git_workspace_snapshot"] as const,
  },
  code: {
    workbench: ["code_workbench_snapshot"] as const,
  },
  tokens: {
    usage: ["token_usage_snapshot"] as const,
    estimates: ["get_project_file_token_estimates"] as const,
  },
  routing: {
    snapshot: (economyMode: string) => ["routing_snapshot", economyMode] as const,
    settings: ["provider_settings"] as const,
    apiEnv: ["get_api_env_diagnostic"] as const,
  },
  models: {
    health: ["model_health_snapshot"] as const,
  },
  system: {
    agents: ["get_system_agents"] as const,
    capabilities: ["get_system_capabilities"] as const,
    peripherals: ["get_system_peripherals"] as const,
    modules: ["get_system_modules"] as const,
  },
  memory: {
    list: (project: string) => ["memory_list", project] as const,
  },
};

/** Wrapper for Tauri invoke that throws on error. */
export async function callCommand<T>(command: string, args?: UnknownRecord): Promise<T> {
  return await invoke<T>(command, args);
}

/** Wrapper for Tauri invoke that returns null on error. */
export async function optionalCommand<T>(command: string, args?: UnknownRecord): Promise<T | null> {
  try {
    return await invoke<T>(command, args);
  } catch {
    return null;
  }
}
