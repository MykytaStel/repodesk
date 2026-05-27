import React from "react";
import { TokenUsageItem } from "../api/api";

type UnknownRecord = Record<string, unknown>;

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

export function formatNumber(value: number | undefined | null): string {
  return typeof value === "number" && Number.isFinite(value) ? value.toLocaleString() : "-";
}

export function formatCost(value: number | undefined | null, currency?: string | null): string {
  if (typeof value !== "number" || !Number.isFinite(value)) return "-";
  return `${value.toFixed(4)} ${currency || "cost_units"}`;
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

export function statusTone(value: string | boolean | undefined | null): "ok" | "warn" | "danger" | "neutral" {
  if (typeof value === "boolean") return value ? "ok" : "warn";
  const lower = String(value || "").toLowerCase();
  if (["working", "ok", "done", "safe", "configured", "not_required"].some((item) => lower.includes(item))) return "ok";
  if (["disabled", "missing", "unreachable", "rate", "warn", "large"].some((item) => lower.includes(item))) return "warn";
  if (["block", "danger", "error", "failed", "too large"].some((item) => lower.includes(item))) return "danger";
  return "neutral";
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

export function MetricCard({ label, value, detail, tone = "neutral" }: { label: string; value: string; detail: string; tone?: "neutral" | "ok" | "warn" }) {
  return <section className={`panel metric ${tone}`}><p className="eyebrow">{label}</p><h2>{value}</h2><p className="muted">{detail}</p></section>;
}

export function UsageRows({ rows, empty }: { rows: any[]; empty: string }) {
  if (rows.length === 0) return <p className="muted">{empty}</p>;
  
  const maxTokens = Math.max(...rows.map(r => r.total_tokens || 0), 1);
  
  return <div className="table-list">{rows.map((row) => {
    const percent = ((row.total_tokens || 0) / maxTokens) * 100;
    return (
      <div className="table-row flex-col items-start gap-sm" key={`${row.provider}-${row.model ?? "all"}`} style={{ paddingBottom: 16 }}>
        <div className="w-full flex justify-between items-center">
          <div><strong>{row.model ? `${row.provider} / ${row.model}` : row.provider}</strong><span>{formatCost(row.estimated_cost_units, row.currency_label)}</span></div>
          <div className="row-meta"><span>in {formatNumber(row.input_tokens)}</span><span>out {formatNumber(row.output_tokens)}</span><strong>{formatNumber(row.total_tokens)}</strong></div>
        </div>
        <div className="w-full" style={{ height: 6, backgroundColor: "rgba(166, 180, 198, 0.1)", borderRadius: 3, overflow: "hidden" }}>
          <div style={{ 
            width: `${percent}%`, 
            height: "100%", 
            background: "linear-gradient(90deg, var(--accent), var(--accent-2))",
            borderRadius: 3,
            transition: "width 0.5s ease-out"
          }} />
        </div>
      </div>
    );
  })}</div>;
}

export function RouteList({ title, items, tone }: { title: string; items: string[]; tone: "ok" | "warn" | "danger" }) {
  return <div className={`route-list ${tone}`}><strong>{title}</strong>{items.slice(0, 4).map((item) => <span key={item}>{item}</span>)}</div>;
}

export function FileGroup({ title, files }: { title: string; files: string[] }) {
  return <section className="panel"><div className="panel-title-row compact"><h2>{title}</h2><span className="pill">{files.length}</span></div><div className="file-list scroll-area small">{files.length ? files.map((file) => <code key={file}>{file}</code>) : <p className="muted">No files.</p>}</div></section>;
}

export function Toggle({ label, checked, onChange }: { label: string; checked: boolean; onChange: (value: boolean) => void }) {
  return <label className="toggle-row"><input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} /><span>{label}</span></label>;
}

export function StartupSkeleton() {
  return <div className="content-grid"><section className="hero-panel skeleton-panel"><div className="skeleton-line big" /><div className="skeleton-line" /><div className="skeleton-line short" /></section><section className="panel skeleton-panel"><div className="skeleton-line" /><div className="skeleton-line" /><div className="skeleton-line short" /></section><section className="panel skeleton-panel"><div className="skeleton-line" /><div className="skeleton-line short" /></section></div>;
}
