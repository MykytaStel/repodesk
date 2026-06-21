import React, { useEffect, useState, useRef } from "react";
import { debugEmitter, type DebugEventDetail, callCommand } from "../api/queries";
import { formatNumber } from "../utils/helpers";

interface ActionRunResult {
  id: string;
  title: string;
  risk: string;
  category: string;
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

interface LogEntry {
  id: string;
  timestamp: string;
  title: string;
  command?: string;
  status: "success" | "error" | "info" | "running";
  durationMs?: number;
  stdout?: string;
  stderr?: string;
  source: "action" | "debug" | "system";
}

interface TerminalPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

function formatOutput(text: string): string {
  if (!text) return "";
  try {
    const parsed = JSON.parse(text);
    return JSON.stringify(parsed, null, 2);
  } catch {
    return text;
  }
}

export function TerminalPanel({ isOpen, onClose }: TerminalPanelProps) {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when new logs arrive
  useEffect(() => {
    if (scrollRef.current && isOpen) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs, isOpen]);

  // Load initial action history
  useEffect(() => {
    callCommand<ActionRunResult[]>("action_history")
      .then((history) => {
        const historyLogs: LogEntry[] = history.map((action) => ({
          id: `action-${action.started_at_ms}-${action.id}`,
          timestamp: new Date(action.started_at_ms).toLocaleTimeString(),
          title: `Action: ${action.title}`,
          command: action.result.command,
          status: action.result.ok ? "success" : "error",
          durationMs: action.finished_at_ms - action.started_at_ms,
          stdout: action.result.stdout,
          stderr: action.result.stderr,
          source: "action",
        }));
        setLogs((prev) => [...historyLogs, ...prev].sort((a, b) => a.id.localeCompare(b.id)));
      })
      .catch(console.error);
  }, []);

  // Listen to real-time debug events
  useEffect(() => {
    const handleDebugEvent = (e: Event) => {
      const detail = (e as CustomEvent<DebugEventDetail>).detail;
      setLogs((prev) => [
        ...prev,
        {
          id: `debug-${detail.id}`,
          timestamp: detail.timestamp,
          title: `Command: ${detail.command}`,
          status: detail.status,
          durationMs: detail.durationMs,
          stdout: detail.preview,
          stderr: detail.error,
          source: "debug",
        },
      ]);
    };

    debugEmitter.addEventListener("debug-command", handleDebugEvent);
    return () => {
      debugEmitter.removeEventListener("debug-command", handleDebugEvent);
    };
  }, []);

  if (!isOpen) return null;

  return (
    <div style={{
      position: "fixed",
      bottom: 0,
      left: 0,
      right: 0,
      height: "45vh",
      backgroundColor: "var(--surface)",
      borderTop: "1px solid var(--border)",
      display: "flex",
      flexDirection: "column",
      zIndex: 900,
      boxShadow: "0 -4px 20px rgba(0,0,0,0.5)",
      fontFamily: "var(--font-mono)",
      fontSize: "13px"
    }}>
      <div style={{
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
        padding: "8px 16px",
        borderBottom: "1px solid var(--border)",
        backgroundColor: "var(--surface-2)"
      }}>
        <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
          <span style={{ fontWeight: 600, color: "var(--text-main)" }}>System Terminal & Logs</span>
          <span className="pill neutral">{logs.length} entries</span>
        </div>
        <div style={{ display: "flex", gap: "8px" }}>
          <button className="ghost-button" style={{ padding: "4px 8px", fontSize: "12px" }} onClick={() => setLogs([])}>Clear</button>
          <button className="ghost-button" style={{ padding: "4px 8px", fontSize: "12px" }} onClick={onClose}>Close</button>
        </div>
      </div>

      <div ref={scrollRef} style={{
        flexGrow: 1,
        overflowY: "auto",
        padding: "16px",
        display: "flex",
        flexDirection: "column",
        gap: "16px",
        backgroundColor: "var(--bg)"
      }}>
        {logs.length === 0 ? (
          <p className="muted" style={{ textAlign: "center", marginTop: "20px" }}>No logs available yet.</p>
        ) : (
          logs.map((log) => (
            <div key={log.id} style={{
              display: "flex",
              flexDirection: "column",
              gap: "4px",
              paddingBottom: "16px",
              borderBottom: "1px solid var(--border)",
            }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
                  <span style={{ color: "var(--text-muted)" }}>{log.timestamp}</span>
                  <span className={`pill ${log.status === "success" ? "ok" : log.status === "error" ? "danger" : "neutral"}`}>
                    {log.source === "action" ? "Action" : "API"}
                  </span>
                  <strong style={{ color: log.status === "error" ? "var(--color-danger)" : "var(--text-main)" }}>
                    {log.title}
                  </strong>
                </div>
                {log.durationMs !== undefined && (
                  <span style={{ color: "var(--text-muted)" }}>{formatNumber(log.durationMs)}ms</span>
                )}
              </div>
              
              {log.command && (
                <div style={{ color: "var(--text-muted)", marginTop: 4 }}>
                  <code>$ {log.command}</code>
                </div>
              )}
              
              {log.stdout && (
                <pre style={{ margin: "4px 0 0 0", padding: "8px", backgroundColor: "var(--surface)", borderRadius: "4px", color: "var(--text-main)", whiteSpace: "pre-wrap", wordBreak: "break-all", fontSize: "12px", border: "1px solid var(--border)" }}>
                  {formatOutput(log.stdout)}
                </pre>
              )}
              
              {log.stderr && (
                <pre style={{ margin: "4px 0 0 0", padding: "8px", backgroundColor: "var(--surface)", borderRadius: "4px", color: "var(--color-danger)", whiteSpace: "pre-wrap", wordBreak: "break-all", fontSize: "12px", border: "1px solid var(--border)" }}>
                  {formatOutput(log.stderr)}
                </pre>
              )}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
