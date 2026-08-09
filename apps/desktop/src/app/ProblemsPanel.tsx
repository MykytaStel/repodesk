import { useMemo, useState, useSyncExternalStore } from "react";
import { requestCodeWorkspaceOpen } from "../shared/api/codeWorkspace";
import {
  getProblemSnapshot,
  subscribeProblems,
  type ProblemDiagnostic,
  type ProblemSeverity,
} from "../shared/api/problems";
import "./styles/problems.css";

type SeverityFilter = "all" | ProblemSeverity;

const SOURCE_LABEL: Record<ProblemDiagnostic["source"], string> = {
  repopilot: "RepoPilot",
  check: "Checks",
  verification: "Verification",
  lsp: "Language Server",
};

function locationLabel(problem: ProblemDiagnostic): string {
  if (!problem.path) return "No file location";
  if (!problem.line) return problem.path;
  return `${problem.path}:${problem.line}${problem.column ? `:${problem.column}` : ""}`;
}

function ProblemRow({ problem }: { problem: ProblemDiagnostic }) {
  const navigable = Boolean(problem.path);
  return (
    <button
      type="button"
      className={`problem-row severity-${problem.severity}`}
      disabled={!navigable}
      onClick={() => {
        if (!problem.path) return;
        requestCodeWorkspaceOpen(problem.path, { line: problem.line, column: problem.column });
      }}
      title={navigable ? `Open ${locationLabel(problem)} in Code` : problem.message}
    >
      <span className="problem-severity-mark" aria-hidden="true" />
      <span className="problem-main">
        <span className="problem-message">{problem.message}</span>
        <span className="problem-meta">
          <span>{SOURCE_LABEL[problem.source]}</span>
          {problem.code ? <code>{problem.code}</code> : null}
          {problem.command ? (
            <span title={problem.command}>
              {problem.source === "lsp" ? problem.command : "from check command"}
            </span>
          ) : null}
        </span>
      </span>
      <code className="problem-location">{locationLabel(problem)}</code>
    </button>
  );
}

export function ProblemsPanel() {
  const snapshot = useSyncExternalStore(subscribeProblems, getProblemSnapshot, getProblemSnapshot);
  const [filter, setFilter] = useState<SeverityFilter>("all");

  const diagnostics = useMemo(
    () => filter === "all"
      ? snapshot.diagnostics
      : snapshot.diagnostics.filter((problem) => problem.severity === filter),
    [filter, snapshot.diagnostics],
  );

  if (snapshot.diagnostics.length === 0) {
    return (
      <div className="problems-empty">
        <strong>No code problems.</strong>
        <span>Rust diagnostics appear live while an .rs file is open. Project tasks and RepoPilot findings use this same list.</span>
      </div>
    );
  }

  return (
    <div className="problems-panel">
      <div className="problems-toolbar">
        <div className="problems-summary" aria-label="Problem counts">
          <span className="danger">{snapshot.errors} errors</span>
          <span className="warn">{snapshot.warnings} warnings</span>
          <span>{snapshot.infos} info</span>
        </div>
        <div className="problems-filters" role="group" aria-label="Filter problems">
          <button type="button" className={filter === "all" ? "active" : ""} onClick={() => setFilter("all")}>All</button>
          <button type="button" className={filter === "error" ? "active" : ""} onClick={() => setFilter("error")}>Errors</button>
          <button type="button" className={filter === "warning" ? "active" : ""} onClick={() => setFilter("warning")}>Warnings</button>
        </div>
      </div>
      <div className="problem-list">
        {diagnostics.length === 0 ? (
          <div className="problems-filter-empty">No problems match this filter.</div>
        ) : diagnostics.map((problem) => <ProblemRow key={problem.id} problem={problem} />)}
      </div>
    </div>
  );
}
