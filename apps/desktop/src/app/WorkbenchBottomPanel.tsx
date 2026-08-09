import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import {
  captureActionDiagnostics,
  clearProblems,
  getProblemSnapshot,
  subscribeProblems,
} from "../shared/api/problems";
import { callCommand, debugEmitter, type DebugEventDetail } from "../shared/api/queries";
import {
  BOTTOM_PANEL_TAB_EVENT,
  type BottomPanelTab,
} from "../shared/api/workbench";
import { useWorkspace } from "../shared/hooks/useWorkspace";
import { formatNumber } from "../shared/utils/helpers";
import { InteractiveTerminal } from "./InteractiveTerminal";
import { ProblemsPanel } from "./ProblemsPanel";
import { TaskRunnerPanel } from "./TaskRunnerPanel";

interface ActionRunResult {
  id: string;
  title: string;
  category?: string;
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

type LogStatus = "success" | "error" | "info" | "running";

interface LogEntry {
  id: string;
  timestamp: string;
  title: string;
  command?: string;
  status: LogStatus;
  durationMs?: number;
  stderr?: string;
  source: "action" | "debug";
}

interface WorkbenchBottomPanelProps {
  open: boolean;
  onClose: () => void;
}

const MAX_PANEL_LOGS = 150;
const MAX_ERROR_CHARS = 4_000;

function boundedText(text: string | undefined): string | undefined {
  if (!text) return undefined;
  if (text.length <= MAX_ERROR_CHARS) return text;
  return `${text.slice(0, MAX_ERROR_CHARS)}\n… [${text.length - MAX_ERROR_CHARS} chars omitted]`;
}

function formatOutput(text: string): string {
  if (!text) return "";
  try {
    return JSON.stringify(JSON.parse(text), null, 2);
  } catch {
    return text;
  }
}

function LogRow({ log }: { log: LogEntry }) {
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
      {log.status === "error" && log.stderr ? <p>{formatOutput(log.stderr)}</p> : null}
    </div>
  );
}

export function WorkbenchBottomPanel({ open, onClose }: WorkbenchBottomPanelProps) {
  const { projectName } = useWorkspace();
  const [activeTab, setActiveTab] = useState<BottomPanelTab>("output");
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const scrollRef = useRef<HTMLDivElement>(null);
  const historyLoaded = useRef(false);
  const historyProject = useRef<string | null>(projectName ?? null);
  const problemSnapshot = useSyncExternalStore(subscribeProblems, getProblemSnapshot, getProblemSnapshot);

  useEffect(() => {
    const onTabRequest = (event: Event) => {
      const tab = (event as CustomEvent<BottomPanelTab>).detail;
      if (tab === "problems" || tab === "tasks" || tab === "output" || tab === "terminal") {
        setActiveTab(tab);
      }
    };
    window.addEventListener(BOTTOM_PANEL_TAB_EVENT, onTabRequest);
    return () => window.removeEventListener(BOTTOM_PANEL_TAB_EVENT, onTabRequest);
  }, []);

  useEffect(() => {
    const nextProject = projectName ?? null;
    if (historyProject.current === nextProject) return;
    historyProject.current = nextProject;
    historyLoaded.current = false;
    setLogs([]);
    clearProblems();
  }, [projectName]);

  // The panel stays mounted so PTY sessions survive hide/show, but historical
  // action output is only requested once the user actually opens the panel.
  useEffect(() => {
    if (!open || historyLoaded.current) return;
    historyLoaded.current = true;

    callCommand<ActionRunResult[]>("action_history")
      .then((history) => {
        const ordered = [...history].sort((left, right) => left.started_at_ms - right.started_at_ms);
        const historyLogs = ordered.map<LogEntry>((action) => ({
          id: `action-${action.started_at_ms}-${action.id}`,
          timestamp: new Date(action.started_at_ms).toLocaleTimeString(),
          title: action.title,
          command: action.result.command,
          status: action.result.ok ? "success" : "error",
          durationMs: action.finished_at_ms - action.started_at_ms,
          stderr: boundedText(action.result.stderr),
          source: "action",
        }));
        setLogs((current) => [...historyLogs, ...current].slice(-MAX_PANEL_LOGS));

        // Rehydrate diagnostics from oldest -> newest so the latest relevant
        // check always owns the current `check` source bucket regardless of the
        // backend's history ordering.
        for (const action of ordered) captureActionDiagnostics(action);
      })
      .catch(() => {
        historyLoaded.current = false;
        // Output is auxiliary shell evidence. A missing history should not break the workspace.
      });
  }, [open, projectName]);

  // Cheap IPC metadata remains useful even while the panel is hidden. Result
  // payloads are not retained here; explicit Debug owns bounded payload capture.
  useEffect(() => {
    const onDebug = (event: Event) => {
      const detail = (event as CustomEvent<DebugEventDetail>).detail;
      setLogs((current) => {
        const next = [
          ...current,
          {
            id: `debug-${detail.id}`,
            timestamp: detail.timestamp,
            title: detail.command,
            status: detail.status,
            durationMs: detail.durationMs,
            stderr: boundedText(detail.error),
            source: "debug" as const,
          },
        ];
        return next.slice(-MAX_PANEL_LOGS);
      });
    };
    debugEmitter.addEventListener("debug-command", onDebug);
    return () => debugEmitter.removeEventListener("debug-command", onDebug);
  }, []);

  useEffect(() => {
    if (open && activeTab === "output" && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs, open, activeTab]);

  const clearActive = () => {
    if (activeTab === "problems") clearProblems();
    else if (activeTab === "output") setLogs([]);
  };

  return (
    <section
      className={`workbench-bottom-panel${open ? "" : " hidden"}`}
      aria-label="Workbench bottom panel"
      aria-hidden={!open}
    >
      <header className="bottom-panel-tabs">
        <div className="bottom-panel-tab-list" role="tablist" aria-label="Bottom panel views">
          <button type="button" className={activeTab === "problems" ? "active" : ""} onClick={() => setActiveTab("problems")}>
            Problems <span>{problemSnapshot.diagnostics.length}</span>
          </button>
          <button type="button" className={activeTab === "tasks" ? "active" : ""} onClick={() => setActiveTab("tasks")}>
            Tasks
          </button>
          <button type="button" className={activeTab === "output" ? "active" : ""} onClick={() => setActiveTab("output")}>
            Output <span>{logs.length}</span>
          </button>
          <button type="button" className={activeTab === "terminal" ? "active" : ""} onClick={() => setActiveTab("terminal")}>
            Terminal
          </button>
        </div>
        <div className="bottom-panel-actions">
          {(activeTab === "problems" || activeTab === "output") ? <button type="button" onClick={clearActive}>Clear</button> : null}
          <button type="button" onClick={onClose} aria-label="Close bottom panel">×</button>
        </div>
      </header>

      {activeTab === "problems" ? (
        <div className="bottom-panel-content problems-host">
          <ProblemsPanel />
        </div>
      ) : null}

      <div className={`bottom-panel-task-host${activeTab === "tasks" ? "" : " bottom-panel-view-hidden"}`}>
        <TaskRunnerPanel active={open && activeTab === "tasks"} onOpenProblems={() => setActiveTab("problems")} />
      </div>

      {activeTab === "output" ? (
        <div className="bottom-panel-content" ref={scrollRef}>
          {logs.length === 0 ? (
            <div className="bottom-panel-empty"><strong>No output yet.</strong><span>RepoDesk actions and API activity will appear here.</span></div>
          ) : (
            logs.map((log) => <LogRow key={log.id} log={log} />)
          )}
        </div>
      ) : null}

      <InteractiveTerminal active={open && activeTab === "terminal"} />
    </section>
  );
}
