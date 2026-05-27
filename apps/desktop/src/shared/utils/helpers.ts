import { ActionItem, UnknownRecord } from "../types/api";

export function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function asRecord(value: unknown): UnknownRecord {
  return isRecord(value) ? value : {};
}

export function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

export function getValue(source: unknown, key: string): unknown {
  return asRecord(source)[key];
}

export function getString(source: unknown, key: string, fallback = "-"): string {
  const value = getValue(source, key);
  if (typeof value === "string" && value.trim()) return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return fallback;
}

export function getNestedString(source: unknown, path: string[], fallback = "-"): string {
  let value: unknown = source;
  for (const segment of path) value = asRecord(value)[segment];
  if (typeof value === "string" && value.trim()) return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return fallback;
}

export function stringifyPreview(value: unknown, max = 4000): string {
  let text: string;
  if (typeof value === "string") text = value;
  else {
    try {
      text = JSON.stringify(value, null, 2);
    } catch {
      text = String(value);
    }
  }
  return text.length > max ? `${text.slice(0, max)}\n\n[truncated]` : text;
}

export function errorToMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return stringifyPreview(error, 1200);
}

export function formatNumber(value: number | undefined | null): string {
  return typeof value === "number" && Number.isFinite(value) ? value.toLocaleString() : "-";
}

export function formatCost(value: number | undefined | null, currency?: string | null): string {
  if (typeof value !== "number" || !Number.isFinite(value)) return "-";
  return `${value.toFixed(4)} ${currency || "cost_units"}`;
}

export function listFromRecord(source: unknown, keys: string[]): string[] {
  const record = asRecord(source);
  for (const key of keys) {
    const value = record[key];
    if (Array.isArray(value)) {
      return value.map((item) => {
        if (typeof item === "string") return item;
        const itemRecord = asRecord(item);
        return getString(itemRecord, "path", getString(itemRecord, "name", stringifyPreview(item, 160)));
      });
    }
  }
  return [];
}

export function gitDirtyCount(git: unknown): number {
  return (
    listFromRecord(git, ["staged", "staged_files"]).length +
    listFromRecord(git, ["unstaged", "unstaged_files", "modified_files"]).length +
    listFromRecord(git, ["untracked", "untracked_files"]).length
  );
}

export function gitIsDirty(git: unknown): boolean {
  const explicit = getValue(git, "is_dirty");
  if (typeof explicit === "boolean") return explicit;
  const clean = getValue(git, "clean");
  if (typeof clean === "boolean") return !clean;
  return gitDirtyCount(git) > 0;
}

export function codeChangedFiles(code: unknown): string[] {
  const direct = listFromRecord(code, ["changed_files", "files"]);
  if (direct.length > 0) return direct;
  return [...listFromRecord(code, ["staged"]), ...listFromRecord(code, ["unstaged"]), ...listFromRecord(code, ["untracked"])];
}

export function normalizeActions(value: unknown): ActionItem[] {
  return asArray(value).map((item, index) => {
    const record = asRecord(item);
    const title = getString(record, "title", getString(record, "label", `Action ${index + 1}`));
    return {
      id: getString(record, "id", `action-${index}`),
      label: title,
      title,
      description: getString(record, "description", "No description."),
      risk: getString(record, "risk", "safe"),
      category: getString(record, "category", "General"),
    };
  });
}

export function statusTone(value: string | boolean | undefined | null): "ok" | "warn" | "danger" | "neutral" {
  if (typeof value === "boolean") return value ? "ok" : "warn";
  const lower = String(value || "").toLowerCase();
  if (["working", "ok", "done", "safe", "configured", "not_required"].some((item) => lower.includes(item))) return "ok";
  if (["disabled", "missing", "unreachable", "rate", "warn", "large"].some((item) => lower.includes(item))) return "warn";
  if (["block", "danger", "error", "failed", "too large"].some((item) => lower.includes(item))) return "danger";
  return "neutral";
}

export function findNextActionId(workflow: unknown, actions: ActionItem[], hasProject: boolean, hasTask: boolean): string {
  if (!hasProject || !hasTask) return "";
  const explicit = getString(workflow, "recommended_action_id", getString(workflow, "next_action_id", ""));
  if (explicit && actions.some((action) => action.id === explicit)) return explicit;
  const preferred = ["smart-context-build", "context-build", "safety-scan-context", "prompt-all", "checks-run", "workflow-next"];
  return preferred.find((id) => actions.some((action) => action.id === id)) ?? actions[0]?.id ?? "";
}

export async function copyToClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const textarea = document.createElement("textarea");
    textarea.value = text;
    document.body.appendChild(textarea);
    textarea.select();
    document.execCommand("copy");
    textarea.remove();
  }
}

