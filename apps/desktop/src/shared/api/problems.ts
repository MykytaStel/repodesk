export type ProblemSeverity = "error" | "warning" | "info";
export type ProblemSource = "repopilot" | "check" | "verification";

export type ProblemDiagnostic = {
  id: string;
  source: ProblemSource;
  severity: ProblemSeverity;
  message: string;
  path: string | null;
  line: number | null;
  column: number | null;
  code: string | null;
  command: string | null;
};

export type ProblemSnapshot = {
  diagnostics: readonly ProblemDiagnostic[];
  errors: number;
  warnings: number;
  infos: number;
};

type CommandLike = {
  ok?: boolean;
  command?: string;
  stdout?: string;
  stderr?: string;
};

type ActionLike = {
  id?: string;
  title?: string;
  category?: string;
  result?: CommandLike;
};

type RepoPilotFindingLike = {
  severity?: string;
  title?: string;
  file?: string | null;
  line?: number | null;
  rule?: string | null;
};

type RepoPilotReportLike = {
  findings?: RepoPilotFindingLike[];
};

const MAX_PROBLEMS = 500;
const MAX_MESSAGE_CHARS = 1_000;
const ANSI_ESCAPE = /\u001b\[[0-9;]*m/g;
const CHECK_ACTION = /(check|verify|verification|test|lint|clippy|cargo|build|tsc|eslint)/i;

const problemEmitter = new EventTarget();
const sourceBuckets = new Map<ProblemSource, ProblemDiagnostic[]>();
let snapshot: ProblemSnapshot = { diagnostics: [], errors: 0, warnings: 0, infos: 0 };

function boundedMessage(value: string): string {
  const clean = value.replace(ANSI_ESCAPE, "").trim();
  if (clean.length <= MAX_MESSAGE_CHARS) return clean;
  return `${clean.slice(0, MAX_MESSAGE_CHARS)}…`;
}

function normalizePath(value: string | undefined | null): string | null {
  if (!value) return null;
  const path = value.trim().replace(/^['"]|['"]$/g, "").replace(/\\/g, "/").replace(/^\.\//, "");
  if (!path || path.startsWith("/") || /^[A-Za-z]:\//.test(path) || path.split("/").includes("..")) return null;
  return path;
}

function problemId(problem: Omit<ProblemDiagnostic, "id">): string {
  return [
    problem.source,
    problem.severity,
    problem.path ?? "-",
    problem.line ?? "-",
    problem.column ?? "-",
    problem.code ?? "-",
    problem.message,
  ].join(":");
}

function diagnostic(input: Omit<ProblemDiagnostic, "id">): ProblemDiagnostic {
  return { ...input, id: problemId(input) };
}

function rebuildSnapshot() {
  const unique = new Map<string, ProblemDiagnostic>();
  for (const bucket of sourceBuckets.values()) {
    for (const problem of bucket) unique.set(problem.id, problem);
  }

  const diagnostics = [...unique.values()]
    .sort((left, right) => {
      const severityRank: Record<ProblemSeverity, number> = { error: 2, warning: 1, info: 0 };
      return severityRank[right.severity] - severityRank[left.severity]
        || (left.path ?? "~").localeCompare(right.path ?? "~")
        || (left.line ?? Number.MAX_SAFE_INTEGER) - (right.line ?? Number.MAX_SAFE_INTEGER);
    })
    .slice(0, MAX_PROBLEMS);

  snapshot = {
    diagnostics,
    errors: diagnostics.filter((item) => item.severity === "error").length,
    warnings: diagnostics.filter((item) => item.severity === "warning").length,
    infos: diagnostics.filter((item) => item.severity === "info").length,
  };
  problemEmitter.dispatchEvent(new Event("change"));
}

export function getProblemSnapshot(): ProblemSnapshot {
  return snapshot;
}

export function subscribeProblems(listener: () => void): () => void {
  problemEmitter.addEventListener("change", listener);
  return () => problemEmitter.removeEventListener("change", listener);
}

export function replaceProblemSource(source: ProblemSource, diagnostics: ProblemDiagnostic[]) {
  sourceBuckets.set(source, diagnostics.slice(0, MAX_PROBLEMS));
  rebuildSnapshot();
}

export function clearProblems(source?: ProblemSource) {
  if (source) sourceBuckets.delete(source);
  else sourceBuckets.clear();
  rebuildSnapshot();
}

function severityFromText(value: string): ProblemSeverity {
  return value.toLowerCase().startsWith("warn") ? "warning" : "error";
}

/**
 * Parse file-backed diagnostics from the human-readable output we already
 * receive from checks. This intentionally recognises a small set of stable
 * shapes instead of pretending arbitrary stderr is a code problem.
 *
 * Supported v0 shapes:
 * - Rust/Cargo/Clippy: `error[...]` followed by `--> path:line:column`
 * - TypeScript: `path(line,column): error TSxxxx: message`
 * - colon style: `path:line:column: error: message`
 */
export function parseCommandDiagnostics(
  output: string,
  options?: { source?: ProblemSource; command?: string | null },
): ProblemDiagnostic[] {
  const source = options?.source ?? "check";
  const command = options?.command ?? null;
  const lines = output.replace(ANSI_ESCAPE, "").split(/\r?\n/);
  const parsed: ProblemDiagnostic[] = [];
  let pending: { severity: ProblemSeverity; message: string; code: string | null } | null = null;

  for (const line of lines) {
    const ts = line.match(/^(.+?)\((\d+),(\d+)\):\s*(error|warning)\s*([A-Z]+\d+)?:?\s*(.+)$/i);
    if (ts) {
      const path = normalizePath(ts[1]);
      parsed.push(diagnostic({
        source,
        severity: severityFromText(ts[4]),
        message: boundedMessage(ts[6]),
        path,
        line: Number(ts[2]),
        column: Number(ts[3]),
        code: ts[5] || null,
        command,
      }));
      pending = null;
      continue;
    }

    const colon = line.match(/^(.+?):(\d+):(\d+):\s*(error|warning)(?:\[([^\]]+)\])?:\s*(.+)$/i);
    if (colon) {
      parsed.push(diagnostic({
        source,
        severity: severityFromText(colon[4]),
        message: boundedMessage(colon[6]),
        path: normalizePath(colon[1]),
        line: Number(colon[2]),
        column: Number(colon[3]),
        code: colon[5] || null,
        command,
      }));
      pending = null;
      continue;
    }

    const rustHeadline = line.match(/^\s*(error|warning)(?:\[([^\]]+)\])?:\s*(.+)$/i);
    if (rustHeadline) {
      pending = {
        severity: severityFromText(rustHeadline[1]),
        message: boundedMessage(rustHeadline[3]),
        code: rustHeadline[2] || null,
      };
      continue;
    }

    const rustLocation = line.match(/^\s*-->\s+(.+?):(\d+):(\d+)\s*$/);
    if (rustLocation && pending) {
      parsed.push(diagnostic({
        source,
        severity: pending.severity,
        message: pending.message,
        path: normalizePath(rustLocation[1]),
        line: Number(rustLocation[2]),
        column: Number(rustLocation[3]),
        code: pending.code,
        command,
      }));
      pending = null;
    }
  }

  return parsed.filter((item) => item.path !== null);
}

export function captureActionDiagnostics(action: ActionLike) {
  const result = action.result;
  if (!result) return;
  const descriptor = [action.id, action.title, action.category, result.command].filter(Boolean).join(" ");
  if (!CHECK_ACTION.test(descriptor)) return;

  const output = `${result.stderr ?? ""}\n${result.stdout ?? ""}`;
  const parsed = parseCommandDiagnostics(output, { source: "check", command: result.command ?? null });
  if (parsed.length > 0) {
    replaceProblemSource("check", parsed);
    return;
  }

  if (result.ok) {
    replaceProblemSource("check", []);
    return;
  }

  const firstUsefulLine = output
    .replace(ANSI_ESCAPE, "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean);
  replaceProblemSource("check", [diagnostic({
    source: "check",
    severity: "error",
    message: boundedMessage(firstUsefulLine || `${action.title || "Check"} failed`),
    path: null,
    line: null,
    column: null,
    code: null,
    command: result.command ?? null,
  })]);
}

export function captureCommandResult(command: string, result: unknown) {
  if (command !== "run_desktop_action" && command !== "run_next_safe_step") return;
  if (!result || typeof result !== "object") return;
  captureActionDiagnostics(result as ActionLike);
}

export function captureRepoPilotProblems(report: RepoPilotReportLike) {
  const diagnostics = (report.findings ?? []).map((finding) => {
    const severity = String(finding.severity ?? "INFO").toUpperCase();
    const mapped: ProblemSeverity = severity === "CRITICAL" || severity === "HIGH"
      ? "error"
      : severity === "MEDIUM"
        ? "warning"
        : "info";
    return diagnostic({
      source: "repopilot",
      severity: mapped,
      message: boundedMessage(finding.title || "RepoPilot finding"),
      path: normalizePath(finding.file),
      line: typeof finding.line === "number" ? finding.line : null,
      column: null,
      code: finding.rule || null,
      command: null,
    });
  });
  replaceProblemSource("repopilot", diagnostics);
}
