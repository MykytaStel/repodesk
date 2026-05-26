import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type TabKey = "workflow" | "ai" | "artifacts" | "actions" | "history" | "debug" | "raw";
type ToastKind = "success" | "error" | "info" | "warning";
type StatusKind = "idle" | "loading" | "success" | "error";

type Toast = {
  id: number;
  kind: ToastKind;
  title: string;
  message: string;
};

type DebugEvent = {
  id: number;
  command: string;
  status: "success" | "error";
  durationMs: number;
  timestamp: string;
  args?: Record<string, unknown>;
  error?: string;
  preview?: string;
};

type CommandResult = {
  ok?: boolean;
  command?: string;
  stdout?: string;
  stderr?: string;
  exit_code?: number | null;
  status?: string;
  message?: string;
  error?: string;
};

type DesktopAction = {
  id: string;
  title?: string;
  label?: string;
  description?: string;
  category?: string;
  risk?: string;
  command_preview?: string;
};

type ActionRunResult = {
  id?: string;
  action_id?: string;
  title?: string;
  risk?: string;
  category?: string;
  started_at_ms?: number;
  finished_at_ms?: number;
  result?: CommandResult;
  status?: string;
  message?: string;
  error?: string;
};

type WorkflowStep = {
  id: string;
  title: string;
  description?: string;
  status?: string;
  action_id?: string | null;
  artifact_kind?: string | null;
  command_preview?: string | null;
  blocker?: string | null;
};

type ArtifactStatus = {
  kind: string;
  title?: string;
  path?: string | null;
  exists?: boolean;
  size_bytes?: number;
};

type ArtifactContent = {
  kind: string;
  title?: string;
  path?: string;
  exists?: boolean;
  content?: string;
  size_bytes?: number;
};

type ProductWorkflowState = {
  generated_at_ms?: number;
  overall_status?: string;
  primary_cta?: string;
  recommended_action_id?: string | null;
  recommended_action_title?: string | null;
  steps?: WorkflowStep[];
  artifacts?: ArtifactStatus[];
  project_ok?: boolean;
  task_ok?: boolean;
  context_ok?: boolean;
  smart_context_ok?: boolean;
  prompts_ok?: boolean;
  checks_ok?: boolean;
  safety_ok?: boolean;
  project_info?: CommandResult;
  task_status?: CommandResult;
  workflow_hint?: CommandResult;
  security_verdict?: CommandResult;
  checks_summary_preview?: string | null;
};

type AiToolProbe = {
  id: string;
  name: string;
  category?: string;
  status: string;
  detection?: string;
  executable_path?: string | null;
  app_path?: string | null;
  local_only?: boolean;
  requires_paid_account?: boolean;
  risk_level?: string;
  notes?: string[];
};

type AiEndpointProbe = {
  id: string;
  name: string;
  url: string;
  status: string;
  local_only?: boolean;
  notes?: string[];
};

type AiDiscoveryReport = {
  generated_at?: string;
  host_os?: string;
  tools?: AiToolProbe[];
  endpoints?: AiEndpointProbe[];
  recommendations?: string[];
  warnings?: string[];
  report_path?: string | null;
};

type Snapshot = Record<string, unknown>;

type ArtifactKind = {
  kind: string;
  label: string;
  hint: string;
};

const tabs: Array<{ key: TabKey; label: string }> = [
  { key: "workflow", label: "Workflow" },
  { key: "ai", label: "AI Discovery" },
  { key: "artifacts", label: "Artifacts" },
  { key: "actions", label: "Actions" },
  { key: "history", label: "History" },
  { key: "debug", label: "Debug" },
  { key: "raw", label: "Raw" },
];

const artifactKinds: ArtifactKind[] = [
  { kind: "smart_context", label: "Smart context", hint: "The preferred compact context for paid agents." },
  { kind: "prompt_codex", label: "Codex prompt", hint: "Patch-oriented prompt." },
  { kind: "prompt_chatgpt", label: "ChatGPT prompt", hint: "Architecture/reasoning prompt." },
  { kind: "prompt_review", label: "Review prompt", hint: "Review and safety prompt." },
  { kind: "checks_summary", label: "Checks summary", hint: "Short failure summary for humans and AI." },
  { kind: "context", label: "Full context", hint: "Full task context, can be large." },
  { kind: "token_estimate", label: "Token estimate", hint: "Budget and token signals." },
];

const defaultSteps: WorkflowStep[] = [
  { id: "project", title: "Project", status: "unknown", description: "Select or add a project." },
  { id: "task", title: "Task", status: "unknown", description: "Create an active task." },
  { id: "context", title: "Context", status: "unknown", description: "Build bounded context." },
  { id: "smart_context", title: "Smart context", status: "unknown", description: "Build compact context." },
  { id: "safety", title: "Safety", status: "unknown", description: "Scan before sharing with AI." },
  { id: "prompts", title: "Prompts", status: "unknown", description: "Generate prompts." },
  { id: "checks", title: "Checks", status: "unknown", description: "Run project checks." },
];

function formatBytes(value?: number): string {
  const bytes = value ?? 0;
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function formatDuration(ms?: number): string {
  if (ms === undefined || Number.isNaN(ms)) return "—";
  if (ms < 1000) return `${Math.round(ms)} ms`;
  return `${(ms / 1000).toFixed(1)} s`;
}

function normalizeStatus(value?: string): string {
  if (!value) return "unknown";
  return value.replace(/_/g, " ");
}

function actionLabel(action: DesktopAction): string {
  return action.title ?? action.label ?? action.id;
}

function commandWasOk(result?: CommandResult): boolean {
  if (!result) return false;
  if (typeof result.ok === "boolean") return result.ok;
  if (typeof result.status === "string") return ["ok", "success", "done", "allow"].includes(result.status.toLowerCase());
  return !result.error;
}

function textFromCommand(result?: CommandResult): string {
  if (!result) return "No output.";
  const parts = [result.message, result.stdout, result.stderr, result.error].filter(Boolean) as string[];
  return parts.join("\n").trim() || (commandWasOk(result) ? "OK" : "No output.");
}

function previewPayload(value: unknown): string {
  try {
    const text = JSON.stringify(value, null, 2);
    return text.length > 1400 ? `${text.slice(0, 1400)}…` : text;
  } catch {
    return String(value);
  }
}

function statusClass(status?: string): string {
  const normalized = (status ?? "unknown").toLowerCase();
  if (["done", "ok", "success", "allow", "available", "found", "running"].some((item) => normalized.includes(item))) return "good";
  if (["current", "warn", "warning", "partial", "bounded"].some((item) => normalized.includes(item))) return "warn";
  if (["block", "error", "failed", "missing", "not_found", "unavailable"].some((item) => normalized.includes(item))) return "bad";
  return "neutral";
}

function StatusBadge({ status }: { status?: string }) {
  return <span className={`statusBadge ${statusClass(status)}`}>{normalizeStatus(status)}</span>;
}

function Spinner() {
  return <span className="spinner" aria-label="loading" />;
}

function ToastStack({ toasts, dismiss }: { toasts: Toast[]; dismiss: (id: number) => void }) {
  return (
    <div className="toastStack">
      {toasts.map((toast) => (
        <button key={toast.id} className={`toast ${toast.kind}`} onClick={() => dismiss(toast.id)}>
          <strong>{toast.title}</strong>
          <span>{toast.message}</span>
        </button>
      ))}
    </div>
  );
}

function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <div className="emptyState">
      <div className="emptyIcon">?</div>
      <h3>{title}</h3>
      <p>{body}</p>
    </div>
  );
}

function OutputBlock({ title, result }: { title: string; result?: CommandResult }) {
  return (
    <section className="panel outputPanel">
      <div className="panelTitleRow">
        <div>
          <h3>{title}</h3>
          <p>{result?.command ?? "No command captured"}</p>
        </div>
        <StatusBadge status={commandWasOk(result) ? "success" : "needs attention"} />
      </div>
      <pre>{textFromCommand(result)}</pre>
    </section>
  );
}

export default function App() {
  const [activeTab, setActiveTab] = useState<TabKey>("workflow");
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [workflow, setWorkflow] = useState<ProductWorkflowState | null>(null);
  const [actions, setActions] = useState<DesktopAction[]>([]);
  const [history, setHistory] = useState<ActionRunResult[]>([]);
  const [aiReport, setAiReport] = useState<AiDiscoveryReport | null>(null);
  const [selectedArtifact, setSelectedArtifact] = useState<ArtifactKind>(artifactKinds[0]);
  const [artifact, setArtifact] = useState<ArtifactContent | null>(null);
  const [lastResult, setLastResult] = useState<ActionRunResult | null>(null);
  const [debugEvents, setDebugEvents] = useState<DebugEvent[]>([]);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [busyLabel, setBusyLabel] = useState<string | null>(null);
  const [globalStatus, setGlobalStatus] = useState<StatusKind>("idle");

  const pushToast = useCallback((kind: ToastKind, title: string, message: string) => {
    const id = Date.now() + Math.floor(Math.random() * 1000);
    setToasts((current) => [{ id, kind, title, message }, ...current].slice(0, 4));
    window.setTimeout(() => {
      setToasts((current) => current.filter((toast) => toast.id !== id));
    }, 6500);
  }, []);

  const dismissToast = useCallback((id: number) => {
    setToasts((current) => current.filter((toast) => toast.id !== id));
  }, []);

  const recordDebug = useCallback((event: Omit<DebugEvent, "id" | "timestamp">) => {
    setDebugEvents((current) => [
      {
        id: Date.now() + Math.floor(Math.random() * 1000),
        timestamp: new Date().toLocaleTimeString(),
        ...event,
      },
      ...current,
    ].slice(0, 80));
  }, []);

  const safeInvoke = useCallback(
    async <T,>(command: string, args?: Record<string, unknown>, options?: { toast?: boolean; label?: string }): Promise<T | null> => {
      const started = performance.now();
      try {
        const data = await invoke<T>(command, args);
        const durationMs = performance.now() - started;
        recordDebug({
          command,
          args,
          status: "success",
          durationMs,
          preview: previewPayload(data),
        });
        if (options?.toast) {
          pushToast("success", options.label ?? command, `Finished in ${formatDuration(durationMs)}`);
        }
        return data;
      } catch (error) {
        const durationMs = performance.now() - started;
        const message = String(error);
        recordDebug({ command, args, status: "error", durationMs, error: message });
        if (options?.toast !== false) {
          pushToast("error", options?.label ?? command, message);
        }
        return null;
      }
    },
    [pushToast, recordDebug],
  );

  const refreshAll = useCallback(async () => {
    setBusyLabel("Refreshing brain state");
    setGlobalStatus("loading");
    const [nextSnapshot, nextWorkflow, nextActions, nextHistory] = await Promise.all([
      safeInvoke<Snapshot>("desktop_snapshot", undefined, { toast: false }),
      safeInvoke<ProductWorkflowState>("product_workflow_state", undefined, { toast: false }),
      safeInvoke<DesktopAction[]>("desktop_actions", undefined, { toast: false }),
      safeInvoke<ActionRunResult[]>("action_history", undefined, { toast: false }),
    ]);

    if (nextSnapshot) setSnapshot(nextSnapshot);
    if (nextWorkflow) setWorkflow(nextWorkflow);
    if (nextActions) setActions(nextActions);
    if (nextHistory) setHistory(nextHistory);

    const ok = Boolean(nextSnapshot || nextWorkflow || nextActions);
    setGlobalStatus(ok ? "success" : "error");
    setBusyLabel(null);
    pushToast(ok ? "success" : "warning", "Refresh complete", ok ? "RepoDesk state was updated." : "Some commands failed. Open Debug tab.");
  }, [pushToast, safeInvoke]);

  const runAiScan = useCallback(async () => {
    setBusyLabel("Scanning local AI systems");
    setGlobalStatus("loading");
    const report = await safeInvoke<AiDiscoveryReport>("ai_discovery_scan", undefined, {
      toast: true,
      label: "AI discovery scan",
    });
    if (report) {
      setAiReport(report);
      setActiveTab("ai");
    }
    setGlobalStatus(report ? "success" : "error");
    setBusyLabel(null);
  }, [safeInvoke]);

  const runAction = useCallback(
    async (actionId: string) => {
      const action = actions.find((item) => item.id === actionId);
      setBusyLabel(action ? `Running: ${actionLabel(action)}` : `Running: ${actionId}`);
      setGlobalStatus("loading");
      const result = await safeInvoke<ActionRunResult>("run_desktop_action", { actionId }, {
        toast: true,
        label: action ? actionLabel(action) : actionId,
      });
      if (result) setLastResult(result);
      setGlobalStatus(result ? "success" : "error");
      setBusyLabel(null);
      await refreshAll();
    },
    [actions, refreshAll, safeInvoke],
  );

  const runNext = useCallback(async () => {
    setBusyLabel("Running next safe step");
    setGlobalStatus("loading");
    const result = await safeInvoke<ActionRunResult>("run_next_safe_step", undefined, {
      toast: true,
      label: "Next safe step",
    });
    if (result) setLastResult(result);
    setGlobalStatus(result ? "success" : "error");
    setBusyLabel(null);
    await refreshAll();
  }, [refreshAll, safeInvoke]);

  const loadArtifact = useCallback(
    async (kind: ArtifactKind) => {
      setSelectedArtifact(kind);
      setBusyLabel(`Loading ${kind.label}`);
      const content = await safeInvoke<ArtifactContent>("read_artifact", { kind: kind.kind }, {
        toast: false,
        label: kind.label,
      });
      setArtifact(content);
      setBusyLabel(null);
      if (content?.exists) {
        pushToast("success", kind.label, "Artifact loaded.");
      } else {
        pushToast("warning", kind.label, "Artifact does not exist yet. Run the related workflow action.");
      }
    },
    [pushToast, safeInvoke],
  );

  useEffect(() => {
    void refreshAll();
  }, [refreshAll]);

  const steps = workflow?.steps?.length ? workflow.steps : defaultSteps;

  const recommendedAction = useMemo(() => {
    if (workflow?.recommended_action_id) {
      return actions.find((action) => action.id === workflow.recommended_action_id) ?? null;
    }
    return actions[0] ?? null;
  }, [actions, workflow?.recommended_action_id]);

  const availableTools = aiReport?.tools?.filter((tool) => statusClass(tool.status) === "good") ?? [];
  const missingTools = aiReport?.tools?.filter((tool) => statusClass(tool.status) === "bad") ?? [];
  const availableEndpoints = aiReport?.endpoints?.filter((endpoint) => statusClass(endpoint.status) === "good") ?? [];

  const statusCards = [
    { label: "Project", value: workflow?.project_ok ? "ready" : "needs setup", status: workflow?.project_ok ? "done" : "warning" },
    { label: "Task", value: workflow?.task_ok ? "ready" : "needs task", status: workflow?.task_ok ? "done" : "warning" },
    { label: "Smart context", value: workflow?.smart_context_ok ? "exists" : "missing", status: workflow?.smart_context_ok ? "done" : "warning" },
    { label: "Prompts", value: workflow?.prompts_ok ? "generated" : "missing", status: workflow?.prompts_ok ? "done" : "warning" },
    { label: "Checks", value: workflow?.checks_ok ? "summary exists" : "not run", status: workflow?.checks_ok ? "done" : "warning" },
    { label: "AI found", value: `${availableTools.length} tools / ${availableEndpoints.length} endpoints`, status: aiReport ? "done" : "unknown" },
  ];

  return (
    <main className="appShell">
      <ToastStack toasts={toasts} dismiss={dismissToast} />

      <header className="hero">
        <div className="heroCopy">
          <div className="eyebrowRow">
            <span className="pulseDot" />
            <span>RepoDesk Desktop</span>
            <StatusBadge status={globalStatus} />
          </div>
          <h1>Control brain for your AI development workflow</h1>
          <p>
            This cockpit should always show what happened, what was found, what failed, and what the next safe step is.
          </p>
        </div>

        <div className="heroPanel">
          <div className="brainStatus">
            {busyLabel ? <Spinner /> : <span className="readyOrb" />}
            <div>
              <strong>{busyLabel ?? "Brain is idle"}</strong>
              <span>{workflow?.primary_cta ?? "Refresh state or create a task to continue."}</span>
            </div>
          </div>
          <div className="heroButtons">
            <button className="button ghost" onClick={() => void refreshAll()} disabled={Boolean(busyLabel)}>
              Refresh
            </button>
            <button className="button ghost" onClick={() => void runAiScan()} disabled={Boolean(busyLabel)}>
              Scan AI
            </button>
            <button className="button primary" onClick={() => void runNext()} disabled={Boolean(busyLabel)}>
              {busyLabel ? "Working…" : workflow?.primary_cta ?? "Do next safe step"}
            </button>
          </div>
          {recommendedAction ? (
            <p className="smallHint">Next mapped action: <strong>{actionLabel(recommendedAction)}</strong></p>
          ) : (
            <p className="smallHint danger">No safe action is currently mapped. Check Debug.</p>
          )}
        </div>
      </header>

      <section className="statusGrid">
        {statusCards.map((card) => (
          <article className="statusCard" key={card.label}>
            <span>{card.label}</span>
            <strong>{card.value}</strong>
            <StatusBadge status={card.status} />
          </article>
        ))}
      </section>

      <nav className="tabs">
        {tabs.map((tab) => (
          <button key={tab.key} className={activeTab === tab.key ? "active" : ""} onClick={() => setActiveTab(tab.key)}>
            {tab.label}
          </button>
        ))}
      </nav>

      {activeTab === "workflow" && (
        <section className="layout twoColumns">
          <div className="panel largePanel">
            <div className="panelTitleRow">
              <div>
                <h2>Workflow timeline</h2>
                <p>Clear state for each step. No silent actions.</p>
              </div>
              <StatusBadge status={workflow?.overall_status} />
            </div>
            <div className="timeline">
              {steps.map((step, index) => (
                <article className={`timelineItem ${statusClass(step.status)}`} key={step.id}>
                  <div className="stepIndex">{index + 1}</div>
                  <div>
                    <div className="timelineHead">
                      <h3>{step.title}</h3>
                      <StatusBadge status={step.status} />
                    </div>
                    <p>{step.description}</p>
                    {step.command_preview ? <code>{step.command_preview}</code> : null}
                    {step.blocker ? <p className="blocker">Blocked: {step.blocker}</p> : null}
                    {step.action_id ? (
                      <button className="button tiny" onClick={() => void runAction(step.action_id as string)} disabled={Boolean(busyLabel)}>
                        Run step action
                      </button>
                    ) : null}
                  </div>
                </article>
              ))}
            </div>
          </div>

          <div className="stack">
            <OutputBlock title="Project info" result={workflow?.project_info} />
            <OutputBlock title="Task status" result={workflow?.task_status} />
            <OutputBlock title="Brain hint" result={workflow?.workflow_hint} />
          </div>
        </section>
      )}

      {activeTab === "ai" && (
        <section className="layout twoColumns">
          <div className="panel largePanel">
            <div className="panelTitleRow">
              <div>
                <h2>AI Discovery</h2>
                <p>Passive scan: PATH lookup, known app paths, localhost endpoints.</p>
              </div>
              <button className="button primary" onClick={() => void runAiScan()} disabled={Boolean(busyLabel)}>
                Scan again
              </button>
            </div>

            {!aiReport ? (
              <EmptyState title="No scan yet" body="Click Scan AI to see which local tools and endpoints RepoDesk can detect." />
            ) : (
              <>
                <div className="summaryStrip">
                  <span><strong>{availableTools.length}</strong> available tools</span>
                  <span><strong>{missingTools.length}</strong> missing tools</span>
                  <span><strong>{availableEndpoints.length}</strong> active endpoints</span>
                  <span><strong>{aiReport.host_os ?? "unknown"}</strong> host</span>
                </div>

                <h3 className="sectionTitle">Tools</h3>
                <div className="tableList">
                  {(aiReport.tools ?? []).map((tool) => (
                    <article className="tableRow" key={tool.id}>
                      <div>
                        <strong>{tool.name}</strong>
                        <span>{tool.executable_path ?? tool.app_path ?? tool.detection ?? "No path detected"}</span>
                      </div>
                      <div className="rowMeta">
                        <StatusBadge status={tool.status} />
                        <span>{tool.category ?? "tool"}</span>
                        <span>{tool.local_only ? "local" : "external"}</span>
                      </div>
                    </article>
                  ))}
                </div>

                <h3 className="sectionTitle">Local endpoints</h3>
                <div className="tableList">
                  {(aiReport.endpoints ?? []).map((endpoint) => (
                    <article className="tableRow" key={endpoint.id}>
                      <div>
                        <strong>{endpoint.name}</strong>
                        <span>{endpoint.url}</span>
                      </div>
                      <StatusBadge status={endpoint.status} />
                    </article>
                  ))}
                </div>
              </>
            )}
          </div>

          <div className="stack">
            <section className="panel">
              <h3>Recommendations</h3>
              {(aiReport?.recommendations?.length ? aiReport.recommendations : ["Run AI scan to generate recommendations."]).map((item, index) => (
                <p className="note" key={`${item}-${index}`}>{item}</p>
              ))}
            </section>
            <section className="panel">
              <h3>Warnings</h3>
              {(aiReport?.warnings?.length ? aiReport.warnings : ["No warnings yet."]).map((item, index) => (
                <p className="note warning" key={`${item}-${index}`}>{item}</p>
              ))}
            </section>
          </div>
        </section>
      )}

      {activeTab === "artifacts" && (
        <section className="layout twoColumns">
          <div className="panel">
            <h2>Artifacts</h2>
            <p>Open generated context, prompts, token estimates and check summaries directly in the UI.</p>
            <div className="artifactButtons">
              {artifactKinds.map((item) => (
                <button
                  key={item.kind}
                  className={`artifactButton ${selectedArtifact.kind === item.kind ? "active" : ""}`}
                  onClick={() => void loadArtifact(item)}
                >
                  <strong>{item.label}</strong>
                  <span>{item.hint}</span>
                </button>
              ))}
            </div>
          </div>

          <div className="panel largePanel">
            <div className="panelTitleRow">
              <div>
                <h2>{artifact?.title ?? selectedArtifact.label}</h2>
                <p>{artifact?.path ?? "Artifact path will appear after loading."}</p>
              </div>
              <StatusBadge status={artifact?.exists ? "found" : "missing"} />
            </div>
            <div className="artifactMeta">Size: {formatBytes(artifact?.size_bytes)}</div>
            {artifact?.content ? <pre className="artifactPre">{artifact.content}</pre> : <EmptyState title="Nothing loaded" body="Select an artifact or generate it from the Workflow tab." />}
          </div>
        </section>
      )}

      {activeTab === "actions" && (
        <section className="layout twoColumns">
          <div className="panel largePanel">
            <div className="panelTitleRow">
              <div>
                <h2>Whitelisted actions</h2>
                <p>These are the only actions exposed to the desktop UI. No arbitrary shell.</p>
              </div>
              <button className="button ghost" onClick={() => void refreshAll()} disabled={Boolean(busyLabel)}>Reload</button>
            </div>
            <div className="actionGrid">
              {actions.map((action) => (
                <article className="actionCard" key={action.id}>
                  <div>
                    <div className="actionHead">
                      <h3>{actionLabel(action)}</h3>
                      <StatusBadge status={action.risk ?? "safe"} />
                    </div>
                    <p>{action.description ?? "No description."}</p>
                    <code>{action.command_preview ?? action.id}</code>
                  </div>
                  <button className="button tiny" onClick={() => void runAction(action.id)} disabled={Boolean(busyLabel)}>Run</button>
                </article>
              ))}
              {!actions.length ? <EmptyState title="No actions loaded" body="Open Debug tab to see if desktop_actions failed." /> : null}
            </div>
          </div>

          <div className="stack">
            <section className="panel">
              <h3>Last result</h3>
              {lastResult ? (
                <>
                  <p><strong>{lastResult.title ?? lastResult.action_id ?? lastResult.id}</strong></p>
                  <StatusBadge status={commandWasOk(lastResult.result) ? "success" : "error"} />
                  <pre>{textFromCommand(lastResult.result)}</pre>
                </>
              ) : (
                <p className="muted">Run an action to see stdout/stderr here.</p>
              )}
            </section>
          </div>
        </section>
      )}

      {activeTab === "history" && (
        <section className="panel largePanel">
          <div className="panelTitleRow">
            <div>
              <h2>Action history</h2>
              <p>Local history of UI-triggered actions.</p>
            </div>
            <button className="button ghost" onClick={() => void refreshAll()} disabled={Boolean(busyLabel)}>Refresh</button>
          </div>
          <div className="tableList">
            {history.map((item, index) => (
              <article className="tableRow" key={`${item.id ?? item.action_id}-${index}`}>
                <div>
                  <strong>{item.title ?? item.action_id ?? item.id ?? "action"}</strong>
                  <span>{item.result?.command ?? item.message ?? "No command captured"}</span>
                </div>
                <div className="rowMeta">
                  <StatusBadge status={commandWasOk(item.result) ? "success" : "error"} />
                  <span>{formatDuration((item.finished_at_ms ?? 0) - (item.started_at_ms ?? 0))}</span>
                </div>
              </article>
            ))}
          </div>
          {!history.length ? <EmptyState title="No history yet" body="Run an action from Workflow or Actions to create history." /> : null}
        </section>
      )}

      {activeTab === "debug" && (
        <section className="layout twoColumns">
          <div className="panel largePanel">
            <div className="panelTitleRow">
              <div>
                <h2>Debug console</h2>
                <p>Every Tauri command call, duration, success/error and payload preview.</p>
              </div>
              <button className="button ghost" onClick={() => setDebugEvents([])}>Clear</button>
            </div>
            <div className="debugList">
              {debugEvents.map((event) => (
                <article className={`debugEvent ${event.status}`} key={event.id}>
                  <div className="debugHead">
                    <strong>{event.command}</strong>
                    <span>{event.timestamp}</span>
                    <StatusBadge status={event.status} />
                    <span>{formatDuration(event.durationMs)}</span>
                  </div>
                  {event.args ? <code>args: {JSON.stringify(event.args)}</code> : null}
                  {event.error ? <pre className="errorPre">{event.error}</pre> : null}
                  {event.preview ? <pre>{event.preview}</pre> : null}
                </article>
              ))}
              {!debugEvents.length ? <EmptyState title="No debug events" body="Refresh, scan AI, or run an action to record debug events." /> : null}
            </div>
          </div>

          <div className="stack">
            <section className="panel">
              <h3>Where to debug?</h3>
              <p className="note">UI actions and Tauri command failures are shown here.</p>
              <p className="note">Rust backend errors appear in the terminal where you ran <code>./scripts/dev-desktop.sh</code>.</p>
              <p className="note">Generated files are under the active task run directory in <code>~/.repodesk/runs/...</code>.</p>
            </section>
            <section className="panel">
              <h3>Fast checks</h3>
              <div className="quickActions">
                <button className="button ghost" onClick={() => void refreshAll()}>Refresh state</button>
                <button className="button ghost" onClick={() => void runAiScan()}>AI scan</button>
                <button className="button ghost" onClick={() => void loadArtifact(artifactKinds[0])}>Load smart context</button>
              </div>
            </section>
          </div>
        </section>
      )}

      {activeTab === "raw" && (
        <section className="layout twoColumns">
          <div className="panel largePanel">
            <h2>Raw snapshot</h2>
            <pre className="artifactPre">{JSON.stringify(snapshot, null, 2)}</pre>
          </div>
          <div className="panel largePanel">
            <h2>Raw workflow</h2>
            <pre className="artifactPre">{JSON.stringify(workflow, null, 2)}</pre>
          </div>
        </section>
      )}
    </main>
  );
}
