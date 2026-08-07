import { useEffect, useMemo, useRef, useState } from "react";
import { callCommand, debugEmitter, type DebugEventDetail } from "../shared/api/queries";
import { formatNumber } from "../shared/utils/helpers";

interface ActionRunResult {
  id: string;
  title: string;
  started_at_ms: number;
  finished_at_ms: number;
  result: {
    ok: boolean;
    command: string;
    stdout: string;
    stderr: string;
    exit_code: number | null;
  };
}

type PanelTab = "problems" | "output" | "terminal";
type LogStatus = "success" | "error" | "info" | "running";

interface LogEntry {
  id: string;
  timestamp: string;
  title: string;
  command?: string;
  status: LogStatus;
  durationMs?: number;
  stdout?: string;
  stderr?: string;
  source: "action" | "debug";
}

interface WorkbenchBottomPanelProps {
  open: boolean;
  onClose: () => void;
}

function formatOutput(text: string): string {
  if (!text) return "";
  try {
    return JSON.stringify(JSON.parse(text), null, 2);
  } catch {
    return text;
  }
}

function LogRow({ log, terminalMode = false }: { log: LogEntry; terminalMode?: boolean }) {
  return (
    <div className={`bottom-panel-log ${log.status === "error" ? " error" : ""}`}>
      <div className="bottom-panel-log-head">
        <span>{log.timestamp}</span>
        <span className={`pill ${log.status === "success" ? "ok" : log.status === "error" ? "danger" : "neutral"}`}>
          {log.source === "action" ? "Action" : "API"}
        </span>
        <strong>{log.title}</strong>
        {log.durationMs !== undefined ? <small>{formatNumber(log.durationMs)}ms</small> : null}
      </div>
      {log.command ? <code className="bottom-panel-command">$ {log.command}</code> : null}
      {terminalMode && log.stdout ? <pre>{formatOutput(log.stdout)}</pre> : null}
      {terminalMode && log.stderr ? <pre className="error-output">{formatOutput(log.stderr)}</pre> : null}
      {!terminalMode && log.status === "error" && log.stderr ? <p>{formatOutput(log.stderr)}</p> : null}
    </div>
  );
}

export function WorkbenchBottomPanel({ open, onClose }: WorkbenchBottomPanelProps) {
  const [activeTab, setActiveTab] = useState<PanelTab>("output");
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    callCommand<ActionRunResult[]>("action_history")
      .then((history) => {
        const historyLogs = history.map<LogEntry>((action) => ({
          id: `action-${action.started_at_ms}-${action.id}`,
          timestamp: new Date(action.started_at_ms).toLocaleTimeString(),
          title: action.title,
          command: action.result.command,
          status: action.result.ok ? "success" : "error",
          durationMs: action.finished_at_ms - action.started_at_ms,
          stdout: action.result.stdout,
          stderr: action.result.stderr,
          source: "action",
        }));
        setLogs(historyLogs);
      })
      .catch(() => {
        // Output is auxiliary shell evidence. A missing history should not break the workspace.
      });
  }, []);

  useEffect(() => {
    const onDebug = (event: Event) => {
      const detail = (event as CustomEvent<DebugEventDetail>).detail;
      setLogs((current) => [
        ...current,
        {
          id: `debug-${detail.id}`,
          timestamp: detail.timestamp,
          title: detail.command,
          status: detail.status,
          durationMs: detail.durationMs,
          stdout: detail.preview,
          stderr: detail.error,
          source: "debug",
        },
      ]);
    };
    debugEmitter.addEventListener("debug-command", onDebug);
    return () => debugEmitter.removeEventListener("debug-command", onDebug);
  }, []);

  useEffect(() => {
    if (open && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs, open, activeTab]);

  const problems = useMemo(() => logs.filter((log) => log.status === "error"), [logs]);

  if (!open) return null;

  return (
    <section className="workbench-bottom-panel" aria-label="Workbench bottom panel">
      <header className="bottom-panel-tabs">
        <div className="bottom-panel-tab-list" role="tablist" aria-label="Bottom panel views">
          <button type="button" className={activeTab === "problems" ? "active" : ""} onClick={() => setActiveTab("problems")}>
            Problems <span>{problems.length}</span>
          </button>
          <button type="button" className={activeTab === "output" ? "active" : ""} onClick={() => setActiveTab("output")}>
            Output <span>{logs.length}</span>
          </button>
          <button type="button" className={activeTab === "terminal" ? "active" : ""} onClick={() => setActiveTab("terminal")}>
            Terminal
          </button>
        </div>
        <div className="bottom-panel-actions">
          <button type="button" onClick={() => setLogs([])}>Clear</button>
          <button type="button" onClick={onClose} aria-label="Close bottom panel">×</button>
        </div>
      </header>

      <div className="bottom-panel-content" ref={scrollRef}>
        {activeTab === "problems" ? (
          problems.length === 0 ? (
            <div className="bottom-panel-empty">
              <strong>No runtime problems captured.</strong>
              <span>Compiler, linter and LSP diagnostics will join this surface in the Problems slice.</span>
            </div>
          ) : (
            problems.map((log) => <LogRow key={log.id} log={log} />)
          )
        ) : activeTab === "output" ? (
          logs.length === 0 ? (
            <div className="bottom-panel-empty"><strong>No output yet.</strong><span>RepoDesk actions and API activity will appear here.</span></div>
          ) : (
            logs.map((log) => <LogRow key={log.id} log={log} />)
          )
        ) : logs.length === 0 ? (
          <div className="bottom-panel-empty">
            <strong>No command transcript yet.</strong>
            <span>This is read-only execution evidence in RD2-07; interactive terminal/task runner comes in its dedicated slice.</span>
          </div>
        ) : (
          logs.filter((log) => log.command || log.stdout || log.stderr).map((log) => <LogRow key={log.id} log={log} terminalMode />)
        )}
      </div>
    </section>
  );
}
