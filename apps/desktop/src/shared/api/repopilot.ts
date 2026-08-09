import { captureRepoPilotProblems } from "./problems";
import { callCommand } from "./queries";

export type Severity = "CRITICAL" | "HIGH" | "MEDIUM" | "LOW" | "INFO";

export interface RepoPilotCounts {
  critical: number;
  high: number;
  medium: number;
  low: number;
  info: number;
}

export interface RepoPilotFinding {
  severity: Severity;
  title: string;
  file?: string | null;
  line?: number | null;
  rule?: string | null;
}

export interface RepoPilotReport {
  available: boolean;
  health_score?: number | null;
  total: number;
  counts: RepoPilotCounts;
  findings: RepoPilotFinding[];
  error?: string | null;
}

export interface RepoPilotTrendPoint {
  timestamp: string;
  health_score?: number | null;
  total: number;
  blocking: number;
  counts: RepoPilotCounts;
}

export interface RepoPilotHistory {
  points: RepoPilotTrendPoint[];
}

export interface FileFindings {
  file: string;
  blocking: number;
  total: number;
  worstRank: number;
  findings: RepoPilotFinding[];
}

const RANK: Record<Severity, number> = { CRITICAL: 4, HIGH: 3, MEDIUM: 2, LOW: 1, INFO: 0 };
export const NO_FILE_KEY = "(no file)";

/** Run a fresh RepoPilot review of the active project's current diff. */
export async function runRepopilotReview(): Promise<RepoPilotReport> {
  const report = await callCommand<RepoPilotReport>("repopilot_findings");
  captureRepoPilotProblems(report);
  return report;
}

/** Read the active task's persisted health trend (oldest first). */
export function getRepopilotHistory(): Promise<RepoPilotHistory> {
  return callCommand<RepoPilotHistory>("repopilot_history");
}

/**
 * Bucket findings per file, ordered by worst severity then name. Mirrors the
 * core `group_by_file` so inline rendering and the blocking-per-file badges stay
 * consistent whether the data comes pre-grouped or as a flat report.
 */
export function groupByFile(report: RepoPilotReport | null | undefined): FileFindings[] {
  if (!report || !report.findings?.length) return [];
  const order: string[] = [];
  const buckets = new Map<string, RepoPilotFinding[]>();
  for (const finding of report.findings) {
    const key = finding.file || NO_FILE_KEY;
    if (!buckets.has(key)) order.push(key);
    buckets.get(key)?.push(finding) ?? buckets.set(key, [finding]);
  }
  return order
    .map((file) => {
      const findings = buckets.get(file) ?? [];
      return {
        file,
        findings,
        total: findings.length,
        blocking: findings.filter((f) => f.severity === "CRITICAL" || f.severity === "HIGH").length,
        worstRank: Math.max(0, ...findings.map((f) => RANK[f.severity] ?? 0)),
      };
    })
    .sort((a, b) => b.worstRank - a.worstRank || a.file.localeCompare(b.file));
}

export function severityTone(severity: Severity): "danger" | "warn" | "neutral" {
  if (severity === "CRITICAL" || severity === "HIGH") return "danger";
  if (severity === "MEDIUM") return "warn";
  return "neutral";
}
