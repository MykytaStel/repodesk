import { invoke } from "@tauri-apps/api/core";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";

type UnknownRecord = Record<string, unknown>;

export const debugEmitter = new EventTarget();

export interface DebugEventDetail {
  id: number;
  command: string;
  args?: UnknownRecord;
  status: "success" | "error";
  durationMs: number;
  timestamp: string;
  preview?: string;
  error?: string;
}

function dispatchDebugEvent(detail: DebugEventDetail) {
  debugEmitter.dispatchEvent(new CustomEvent("debug-command", { detail }));
}


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
    costTrend: ["token_cost_trend"] as const,
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
  projectAi: {
    scan: (project: string) => ["project_ai_scan", project] as const,
  },
  memory: {
    list: (project: string) => ["memory_list", project] as const,
    proposals: (project: string) => ["memory_proposals", project] as const,
    preview: (project: string) => ["memory_brain_preview", project] as const,
  },
  orchestrate: {
    status: ["orchestrate_status"] as const,
    runs: ["orchestration_runs"] as const,
    timeline: ["task_timeline"] as const,
    executors: ["coding_agent_executors"] as const,
    worktrees: ["orchestrate_worktrees"] as const,
  },
  outcomes: {
    list: ["outcomes_list"] as const,
    stats: ["outcomes_stats"] as const,
  },
  audit: {
    recent: (limit: number) => ["audit_recent", limit] as const,
    verify: ["audit_verify"] as const,
  },
};

/** Wrapper for Tauri invoke that throws on error. */
export async function callCommand<T>(command: string, args?: UnknownRecord): Promise<T> {
  const started = performance.now();
  try {
    const result = await invoke<T>(command, args);
    const durationMs = Math.round(performance.now() - started);
    dispatchDebugEvent({
      id: Date.now() + Math.random(),
      command,
      args,
      status: "success",
      durationMs,
      timestamp: new Date().toLocaleTimeString(),
      preview: typeof result === "string" ? result : JSON.stringify(result, null, 2),
    });
    return result;
  } catch (error: any) {
    const durationMs = Math.round(performance.now() - started);
    dispatchDebugEvent({
      id: Date.now() + Math.random(),
      command,
      args,
      status: "error",
      durationMs,
      timestamp: new Date().toLocaleTimeString(),
      error: typeof error === "string" ? error : error?.message || String(error),
    });
    throw error;
  }
}

/** Wrapper for Tauri invoke that returns null on error. */
export async function optionalCommand<T>(command: string, args?: UnknownRecord): Promise<T | null> {
  try {
    return await callCommand<T>(command, args);
  } catch {
    return null;
  }
}
