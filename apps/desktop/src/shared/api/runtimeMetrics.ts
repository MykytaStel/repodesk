export type RuntimeCommandMetric = {
  command: string;
  calls: number;
  errors: number;
  total_ms: number;
  max_ms: number;
  last_ms: number;
};

export type RuntimeMetricsSnapshot = {
  started_at: string;
  total_calls: number;
  total_errors: number;
  total_ms: number;
  tracked_commands: number;
  commands: RuntimeCommandMetric[];
};

const MAX_TRACKED_COMMANDS = 96;
let startedAt = new Date().toISOString();
const metrics = new Map<string, RuntimeCommandMetric>();
let totalCalls = 0;
let totalErrors = 0;
let totalMs = 0;

function metricKey(command: string): string {
  if (metrics.has(command)) return command;
  if (metrics.size < MAX_TRACKED_COMMANDS - 1) return command;
  return "__other__";
}

export function recordRuntimeMetric(
  command: string,
  durationMs: number,
  status: "success" | "error",
): void {
  const safeDuration = Number.isFinite(durationMs) ? Math.max(0, durationMs) : 0;
  totalCalls += 1;
  totalMs += safeDuration;
  if (status === "error") totalErrors += 1;

  const key = metricKey(command);
  const current = metrics.get(key) ?? {
    command: key,
    calls: 0,
    errors: 0,
    total_ms: 0,
    max_ms: 0,
    last_ms: 0,
  };

  current.calls += 1;
  current.total_ms += safeDuration;
  current.last_ms = safeDuration;
  current.max_ms = Math.max(current.max_ms, safeDuration);
  if (status === "error") current.errors += 1;
  metrics.set(key, current);
}

export function getRuntimeMetricsSnapshot(): RuntimeMetricsSnapshot {
  return {
    started_at: startedAt,
    total_calls: totalCalls,
    total_errors: totalErrors,
    total_ms: totalMs,
    tracked_commands: metrics.size,
    commands: [...metrics.values()]
      .map((metric) => ({ ...metric }))
      .sort((a, b) => b.total_ms - a.total_ms || b.calls - a.calls || a.command.localeCompare(b.command)),
  };
}

export function resetRuntimeMetrics(): void {
  metrics.clear();
  totalCalls = 0;
  totalErrors = 0;
  totalMs = 0;
  startedAt = new Date().toISOString();
}
