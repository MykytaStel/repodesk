import { invoke } from "@tauri-apps/api/core";
import { debugEmitter, type DebugEventDetail } from "../api/queries";

/**
 * Normalized application error.
 *
 * Tauri commands today return plain `Err(String)`, but the backend already
 * defines a richer `ErrorPayload { category, message, retryable, detail }`
 * (see `commands/journal.rs`). `normalizeError` understands both shapes plus
 * raw `Error`/strings, so the UI has one consistent error model regardless of
 * source — and it keeps working if commands start returning structured errors.
 */
export interface AppError {
  message: string;
  /** configuration | provider_transient | security_block | resource_limit | internal */
  category: string;
  retryable: boolean;
  detail?: unknown;
  stack?: string;
}

const KNOWN_CATEGORIES = [
  "configuration",
  "provider_transient",
  "security_block",
  "resource_limit",
  "internal",
] as const;

export function normalizeError(raw: unknown): AppError {
  // Structured ErrorPayload returned by a Tauri command.
  if (raw && typeof raw === "object" && !(raw instanceof Error)) {
    const obj = raw as Record<string, unknown>;
    if (typeof obj.message === "string" && typeof obj.category === "string") {
      return {
        message: obj.message,
        category: KNOWN_CATEGORIES.includes(obj.category as never) ? obj.category : "internal",
        retryable: Boolean(obj.retryable),
        detail: obj.detail,
      };
    }
  }
  if (raw instanceof Error) {
    return {
      message: raw.message || String(raw),
      category: guessCategory(raw.message),
      retryable: isRetryableText(raw.message),
      stack: raw.stack,
    };
  }
  if (typeof raw === "string") {
    return { message: raw, category: guessCategory(raw), retryable: isRetryableText(raw) };
  }
  return { message: safeString(raw), category: "internal", retryable: false };
}

/** Best-effort category inference from a message string (when not structured). */
function guessCategory(message = ""): string {
  const m = message.toLowerCase();
  if (/(no active project|not set|not found|invalid|configure|missing)/.test(m)) return "configuration";
  if (/(rate.?limit|unreachable|unavailable|timeout|connect|429|offline)/.test(m)) return "provider_transient";
  if (/(secret|credential|blocked|sandbox|denied|forbidden)/.test(m)) return "security_block";
  if (/(too large|budget|exceeds|over the limit|token limit)/.test(m)) return "resource_limit";
  return "internal";
}

function isRetryableText(message = ""): boolean {
  return /(rate.?limit|unreachable|unavailable|timeout|connect|429|offline)/i.test(message);
}

function safeString(value: unknown): string {
  if (value === null || value === undefined) return "Unknown error";
  try {
    return typeof value === "string" ? value : JSON.stringify(value);
  } catch {
    return String(value);
  }
}

/**
 * Central error sink. Routes every error to three places:
 *  1. the console (with a `[RepoDesk:scope]` tag),
 *  2. the in-app Debug tab (via the shared debug emitter),
 *  3. the persistent event journal (`log_ui_event`, best-effort).
 *
 * Returns the normalized error so callers can also render it.
 */
export function reportError(scope: string, raw: unknown, meta?: Record<string, string>): AppError {
  const err = normalizeError(raw);

  console.error(`[RepoDesk:${scope}]`, err.message, err.detail ?? "", err.stack ?? "");

  const detail: DebugEventDetail = {
    id: Date.now() + Math.random(),
    command: `error:${scope}`,
    status: "error",
    durationMs: 0,
    timestamp: new Date().toLocaleTimeString(),
    error: `[${err.category}] ${err.message}`,
  };
  debugEmitter.dispatchEvent(new CustomEvent("debug-command", { detail }));

  const metadata: [string, string][] = [
    ["category", err.category],
    ["scope", scope],
    ...Object.entries(meta ?? {}),
  ];
  void invoke("log_ui_event", {
    input: {
      module_name: scope.slice(0, 80) || "ui",
      level: "error",
      message: (err.message || "Unknown error").slice(0, 1000),
      metadata,
    },
  }).catch(() => {
    /* journal is best-effort; never throw from the error reporter */
  });

  return err;
}
