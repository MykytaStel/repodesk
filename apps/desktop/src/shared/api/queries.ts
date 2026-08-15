import { invoke } from "@tauri-apps/api/core";
import { captureCommandResult } from "./problems";
import { recordRuntimeMetric } from "./runtimeMetrics";

type UnknownRecord = Record<string, unknown>;

export const debugEmitter = new EventTarget();

export interface DebugEventDetail {
  id: number;
  command: string;
  status: "success" | "error";
  durationMs: number;
  timestamp: string;
  preview?: string;
  error?: string;
}

const MAX_DEBUG_STRING_CHARS = 2_000;
const MAX_DEBUG_ARRAY_ITEMS = 16;
const MAX_DEBUG_OBJECT_KEYS = 24;
const MAX_DEBUG_DEPTH = 3;
const SENSITIVE_DEBUG_KEY = /(content|source|prompt|secret|password|authorization|api[_-]?key|token)/i;

let debugPayloadConsumers = 0;
let debugSequence = 0;

/**
 * Enable bounded IPC result previews while an explicit Debug consumer is
 * mounted. Normal product surfaces still receive cheap timing/status events,
 * but do not pay to serialize command results they never render.
 */
export function acquireDebugPayloadCapture(): () => void {
  debugPayloadConsumers += 1;
  let released = false;
  return () => {
    if (released) return;
    released = true;
    debugPayloadConsumers = Math.max(0, debugPayloadConsumers - 1);
  };
}

function truncateDebugString(value: string): string {
  if (value.length <= MAX_DEBUG_STRING_CHARS) return value;
  return `${value.slice(0, MAX_DEBUG_STRING_CHARS)}\n… [${value.length - MAX_DEBUG_STRING_CHARS} chars omitted]`;
}

function summarizeForDebug(value: unknown, depth = 0): unknown {
  if (value == null || typeof value === "number" || typeof value === "boolean") return value;
  if (typeof value === "string") return truncateDebugString(value);
  if (typeof value !== "object") return String(value);

  if (depth >= MAX_DEBUG_DEPTH) {
    if (Array.isArray(value)) return `[Array(${value.length})]`;
    return "[Object]";
  }

  if (Array.isArray(value)) {
    const items = value.slice(0, MAX_DEBUG_ARRAY_ITEMS).map((item) => summarizeForDebug(item, depth + 1));
    if (value.length > MAX_DEBUG_ARRAY_ITEMS) {
      items.push(`… [${value.length - MAX_DEBUG_ARRAY_ITEMS} items omitted]`);
    }
    return items;
  }

  const record = value as UnknownRecord;
  const keys = Object.keys(record);
  const summarized: UnknownRecord = {};
  for (const key of keys.slice(0, MAX_DEBUG_OBJECT_KEYS)) {
    summarized[key] = SENSITIVE_DEBUG_KEY.test(key)
      ? "[omitted]"
      : summarizeForDebug(record[key], depth + 1);
  }
  if (keys.length > MAX_DEBUG_OBJECT_KEYS) {
    summarized.__omitted_keys = keys.length - MAX_DEBUG_OBJECT_KEYS;
  }
  return summarized;
}

function debugPreview(result: unknown): string | undefined {
  if (debugPayloadConsumers === 0) return undefined;
  if (typeof result === "string") return truncateDebugString(result);
  try {
    return JSON.stringify(summarizeForDebug(result), null, 2);
  } catch {
    return "[Debug preview unavailable]";
  }
}

function dispatchDebugEvent(detail: Omit<DebugEventDetail, "id">) {
  debugSequence += 1;
  debugEmitter.dispatchEvent(new CustomEvent("debug-command", { detail: { ...detail, id: debugSequence } }));
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
    settings: ["provider_preferences"] as const,
    apiEnv: ["get_api_env_diagnostic"] as const,
  },
  credentials: {
    status: ["credential_status"] as const,
  },
  models: {
    health: ["model_health_snapshot"] as const,
  },
  recovery: {
    snapshot: ["recovery_snapshot"] as const,
    history: ["recovery_history"] as const,
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
    recordRuntimeMetric(command, durationMs, "success");
    captureCommandResult(command, result);
    dispatchDebugEvent({
      command,
      status: "success",
      durationMs,
      timestamp: new Date().toLocaleTimeString(),
      preview: debugPreview(result),
    });
    return result;
  } catch (error: any) {
    const durationMs = Math.round(performance.now() - started);
    recordRuntimeMetric(command, durationMs, "error");
    dispatchDebugEvent({
      command,
      status: "error",
      durationMs,
      timestamp: new Date().toLocaleTimeString(),
      error: typeof error === "string" ? truncateDebugString(error) : truncateDebugString(error?.message || String(error)),
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
