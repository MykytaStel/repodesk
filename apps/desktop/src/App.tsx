import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type TabKey = "workflow" | "setup" | "ai" | "artifacts" | "actions" | "history" | "debug" | "raw";
type ToastKind = "success" | "error" | "info" | "warning";
type CommandStatus = "idle" | "loading" | "success" | "error";

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

type ProjectAddInput = {
  name: string;
  path: string;
  project_type: string;
};

type Snapshot = Record<string, unknown>;

type ArtifactKind = {
  kind: string;
  label: string;
  hint: string;
};

const tabs: Array<{ key: TabKey; label: string }> = [
  { key: "workflow", label: "Workflow" },
  { key: "setup", label: "Setup" },
  { key: "ai", label: "AI Discovery" },
  { key: "artifacts", label: "Artifacts" },
  { key: "actions", label: "Actions" },
  { key: "history", label: "History" },
  { key: "debug", label: "Debug" },
  { key: "raw", label: "Raw" },
];

const artifactKinds: ArtifactKind[] = [
  { kind: "smart_context", label: "Smart context", hint: "Compact context for paid agents." },
  { kind: "prompt_codex", label: "Codex prompt", hint: "Patch-oriented prompt." },
  { kind: "prompt_chatgpt", label: "ChatGPT prompt", hint: "Reasoning and architecture prompt." },
  { kind: "prompt_review", label: "Review prompt", hint: "Review and safety prompt." },
  { kind: "checks_summary", label: "Checks summary", hint: "Short failure summary." },
  { kind: "context", label: "Full context", hint: "Full task context, can be large." },
  { kind: "token_estimate", label: "Token estimate", hint: "Budget and token signals." },
];

const fallbackSteps: WorkflowStep[] = [
  { id: "project", title: "Project", status: "unknown", description: "Connect or activate a project." },
  { id: "task", title: "Task", status: "unknown", description: "Create an active task." },
  { id: "context", title: "Context", status: "unknown", description: "Build bounded context." },
  { id: "smart_context", title: "Smart context", status: "unknown", description: "Build compact context." },
  { id: "safety", title: "Safety", status: "unknown", description: "Scan before sharing with AI." },
  { id: "prompts", title: "Prompts", status: "unknown", description: "Generate prompts." },
  { id: "checks", title: "Checks", status: "unknown", description: "Run project checks." },
];

function normalizeStatus(value?: string): string {
  if (!value) return "unknown";
  return value.replace(/_/g, " ");
}

function formatDuration(ms?: number): string {
  if (ms === undefined || Number.isNaN(ms)) return "—";
  if (ms < 1000) return `${Math.round(ms)} ms`;
  return `${(ms / 1000).toFixed(1)} s`;
}

function formatBytes(value?: number): string {
  const bytes = value ?? 0;
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function actionLabel(action: DesktopAction): string {
  return action.title ?? action.label ?? action.id;
}

function commandWasOk(result?: CommandResult): boolean {
  if (!result) return false;
  if (typeof result.ok === "boolean") return result.ok;
  if (typeof result.status === "string") {
    return ["ok", "success", "done", "allow"].includes(result.status.toLowerCase());
  }
  return !result.error;
}

function resultText(result?: CommandResult): string {
  if (!result) return "No output.";
  const parts = [result.message, result.stdout, result.stderr, result.error].filter(Boolean) as string[];
  return parts.join("\n").trim() || (commandWasOk(result) ? "OK" : "No output.");
}

function previewPayload(value: unknown): string {
  try {
    const text = JSON.stringify(value, null, 2);
    return text.length > 1600 ? `${text.slice(0, 1600)}…` : text;
  } catch {
    return String(value);
  }
}

function statusClass(status?: string): string {
  const normalized = (status ?? "unknown").toLowerCase();
  if (["done", "ok", "success", "allow", "available", "found", "running", "ready"].some((item) => normalized.includes(item))) return "good";
  if (["current", "warn", "warning", "partial", "bounded", "needs"].some((item) => normalized.includes(item))) return "warn";
  if (["block", "error", "failed", "missing", "not_found", "unavailable"].some((item) => normalized.includes(item))) return "bad";
  return "neutral";
}

function StatusBadge({ status }: { status?: string }) {
  return <span className={`statusBadge ${statusClass(status)}`}>{normalizeStatus(status)}</span>;
}

function Spinner({ label }: { label?: string }) {
  return (
    <span className="spinnerWrap">
      <span className="spinner" />
      {label ? <span>{label}</span> : null}
    </span>
  );
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
      <pre>{resultText(result)}</pre>
    </section>
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

function SkeletonCards() {
  return (
    <div className="skeletonGrid">
      {Array.from({ length: 6 }).map((_, index) => (
        <div className="skeletonCard" key={index}>
          <span />
          <strong />
          <small />
        </div>
      ))}
    </div>
  );
}

export default function App() {
  const [activeTab, setActiveTab] = useState<TabKey>(() => (localStorage.getItem("repodesk.activeTab") as TabKey) || "workflow");
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [workflow, setWorkflow] = useState<ProductWorkflowState | null>(null);
  const [actions, setActions] = useState<DesktopAction[]>([]);
  const [history, setHistory] = useState<ActionRunResult[]>([]);
  const [aiReport, setAiReport] = useState<AiDiscoveryReport | null>(null);
  const [artifact, setArtifact] = useState<ArtifactContent | null>(null);
  const [selectedArtifact, setSelectedArtifact] = useState<ArtifactKind>(artifactKinds[0]);
  const [lastResult, setLastResult] = useState<ActionRunResult | null>(null);
  const [debugEvents, setDebugEvents] = useState<DebugEvent[]>([]);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [busyLabel, setBusyLabel] = useState<string | null>("Booting RepoDesk desktop");
  const [bootStatus, setBootStatus] = useState<CommandStatus>("loading");
  const [projectName, setProjectName] = useState("repodesk");
  const [projectPath, setProjectPath] = useState("");
  const [projectType, setProjectType] = useState("rust-cli");
  const [switchProjectName, setSwitchProjectName] = useState("repodesk");
  const [taskTitle, setTaskTitle] = useState("Continue RepoDesk product workflow MVP");

  useEffect(() => {
    localStorage.setItem("repodesk.activeTab", activeTab);
  }, [activeTab]);

  const pushToast = useCallback((kind: ToastKind, title: string, message: string) => {
    const id = Date.now() + Math.floor(Math.random() * 1000);
    setToasts((current) => [{ id, kind, title, message }, ...current].slice(0, 5));
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
    ].slice(0, 100));
  }, []);

  const safeInvoke = useCallback(
    async <T,>(command: string, args?: Record<string, unknown>, options?: { toast?: boolean; label?: string }): Promise<T | null> => {
      const started = performance.now();
      try {
        const data = await invoke<T>(command, args);
        const durationMs = performance.now() - started;
        recordDebug({ command, args, status: "success", durationMs, preview: previewPayload(data) });
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

  const refreshAll = useCallback(async (quiet = false) => {
    setBusyLabel("Refreshing RepoDesk brain state");
    setBootStatus("loading");
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
    setBootStatus(ok ? "success" : "error");
    setBusyLabel(null);
    if (!quiet) {
      pushToast(ok ? "success" : "warning", "Refresh complete", ok ? "State updated." : "Some commands failed. Open Debug.");
    }
  }, [pushToast, safeInvoke]);

  useEffect(() => {
    void refreshAll(true);
  }, [refreshAll]);

  const runAiScan = useCallback(async () => {
    setBusyLabel("Scanning installed AI tools and local endpoints");
    const report = await safeInvoke<AiDiscoveryReport>("ai_discovery_scan", undefined, { toast: true, label: "AI discovery" });
    if (report) {
      setAiReport(report);
      setActiveTab("ai");
    }
    setBusyLabel(null);
  }, [safeInvoke]);

  const runAction = useCallback(async (actionId: string) => {
    const action = actions.find((item) => item.id === actionId);
    setBusyLabel(action ? `Running ${actionLabel(action)}` : `Running ${actionId}`);
    const result = await safeInvoke<ActionRunResult>("run_desktop_action", { actionId }, { toast: true, label: action ? actionLabel(action) : actionId });
    if (result) setLastResult(result);
    setBusyLabel(null);
    await refreshAll(true);
  }, [actions, refreshAll, safeInvoke]);

  const runNext = useCallback(async () => {
    setBusyLabel("Running next safe workflow step");
    const result = await safeInvoke<ActionRunResult>("run_next_safe_step", undefined, { toast: true, label: "Next safe step" });
    if (result) setLastResult(result);
    setBusyLabel(null);
    await refreshAll(true);
  }, [refreshAll, safeInvoke]);

  const loadArtifact = useCallback(async (kind: ArtifactKind) => {
    setSelectedArtifact(kind);
    setBusyLabel(`Loading ${kind.label}`);
    const content = await safeInvoke<ArtifactContent>("read_artifact", { kind: kind.kind }, { toast: false });
    setArtifact(content);
    setBusyLabel(null);
    if (content?.exists) pushToast("success", kind.label, "Loaded.");
    else pushToast("warning", kind.label, "Missing. Run the related workflow action.");
  }, [pushToast, safeInvoke]);

  const connectProject = useCallback(async () => {
    const input: ProjectAddInput = { name: projectName.trim(), path: projectPath.trim(), project_type: projectType.trim() };
    if (!input.name || !input.path) {
      pushToast("warning", "Project setup", "Project name and path are required.");
      return;
    }
    setBusyLabel("Connecting project");
    const added = await safeInvoke<CommandResult>("project_add", { input }, { toast: true, label: "Add project" });
    if (added) await safeInvoke<CommandResult>("project_use", { name: input.name }, { toast: true, label: "Use project" });
    setBusyLabel(null);
    await refreshAll(true);
  }, [projectName, projectPath, projectType, pushToast, refreshAll, safeInvoke]);

  const activateProject = useCallback(async () => {
    if (!switchProjectName.trim()) {
      pushToast("warning", "Project switch", "Project name is required.");
      return;
    }
    setBusyLabel("Switching active project");
    await safeInvoke<CommandResult>("project_use", { name: switchProjectName.trim() }, { toast: true, label: "Switch project" });
    setBusyLabel(null);
    await refreshAll(true);
  }, [pushToast, refreshAll, safeInvoke, switchProjectName]);

  const createTask = useCallback(async () => {
    if (!taskTitle.trim()) {
      pushToast("warning", "Task setup", "Task title is required.");
      return;
    }
    setBusyLabel("Creating task");
    await safeInvoke<CommandResult>("task_new", { title: taskTitle.trim() }, { toast: true, label: "Create task" });
    setBusyLabel(null);
    await refreshAll(true);
  }, [pushToast, refreshAll, safeInvoke, taskTitle]);

  const steps = workflow?.steps?.length ? workflow.steps : fallbackSteps;
  const recommendedAction = useMemo(() => {
    if (workflow?.recommended_action_id) return actions.find((action) => action.id === workflow.recommended_action_id) ?? null;
    return actions[0] ?? null;
  }, [actions, workflow?.recommended_action_id]);

  const foundTools = aiReport?.tools?.filter((tool) => statusClass(tool.status) === "good") ?? [];
  const missingTools = aiReport?.tools?.filter((tool) => statusClass(tool.status) === "bad") ?? [];
  const foundEndpoints = aiReport?.endpoints?.filter((endpoint) => statusClass(endpoint.status) === "good") ?? [];

  const statusCards = [
    { label: "Project", value: workflow?.project_ok ? "ready" : "needs setup", status: workflow?.project_ok ? "done" : "warning" },
    { label: "Task", value: workflow?.task_ok ? "ready" : "needs task", status: workflow?.task_ok ? "done" : "warning" },
    { label: "Smart context", value: workflow?.smart_context_ok ? "exists" : "missing", status: workflow?.smart_context_ok ? "done" : "warning" },
    { label: "Prompts", value: workflow?.prompts_ok ? "generated" : "missing", status: workflow?.prompts_ok ? "done" : "warning" },
    { label: "Checks", value: workflow?.checks_ok ? "summary exists" : "not run", status: workflow?.checks_ok ? "done" : "warning" },
    { label: "AI discovery", value: aiReport ? `${foundTools.length} tools / ${foundEndpoints.length} endpoints` : "not scanned", status: aiReport ? "done" : "unknown" },
  ];

  return (
    <main className="appShell">
      <ToastStack toasts={toasts} dismiss={(id) => setToasts((current) => current.filter((toast) => toast.id !== id))} />

      {busyLabel ? (
        <div className="loadingOverlay">
          <div className="loadingBox">
            <Spinner />
            <strong>{busyLabel}</strong>
            <span>RepoDesk is running a local command. Open Debug for details.</span>
          </div>
        </div>
      ) : null}

      <header className="hero">
        <div>
          <p className="eyebrow">RepoDesk Desktop</p>
          <h1>Local AI workflow cockpit</h1>
          <p>
            Connect a project, create a task, build bounded context, scan local AI tools, generate prompts, run checks, and keep every action visible.
          </p>
        </div>
        <div className="heroActions">
          <button className="primary" onClick={runNext} disabled={Boolean(busyLabel)}>
            {busyLabel ? <Spinner label="Working" /> : workflow?.primary_cta ?? "Do next safe step"}
          </button>
          <button onClick={() => refreshAll(false)} disabled={Boolean(busyLabel)}>Refresh</button>
          <button onClick={runAiScan} disabled={Boolean(busyLabel)}>Scan AI</button>
        </div>
      </header>

      <section className="statusGrid">
        {bootStatus === "loading" && !workflow ? <SkeletonCards /> : statusCards.map((card) => (
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

      {activeTab === "workflow" ? (
        <section className="contentGrid two">
          <div className="panel largePanel">
            <div className="panelTitleRow">
              <div>
                <h2>Workflow timeline</h2>
                <p>The app should always tell you what has happened, what is blocked, and what to do next.</p>
              </div>
              <StatusBadge status={workflow?.overall_status ?? "unknown"} />
            </div>
            <div className="timeline">
              {steps.map((step, index) => (
                <article className={`timelineStep ${statusClass(step.status)}`} key={step.id}>
                  <div className="stepIndex">{index + 1}</div>
                  <div>
                    <div className="stepHeader">
                      <strong>{step.title}</strong>
                      <StatusBadge status={step.status} />
                    </div>
                    <p>{step.description ?? "No description."}</p>
                    {step.blocker ? <small className="badText">Blocked: {step.blocker}</small> : null}
                    {step.command_preview ? <code>{step.command_preview}</code> : null}
                  </div>
                </article>
              ))}
            </div>
          </div>

          <aside className="panel">
            <h2>Recommended next action</h2>
            {recommendedAction ? (
              <div className="actionFocus">
                <StatusBadge status={recommendedAction.risk ?? "bounded"} />
                <h3>{actionLabel(recommendedAction)}</h3>
                <p>{recommendedAction.description ?? "No description."}</p>
                {recommendedAction.command_preview ? <code>{recommendedAction.command_preview}</code> : null}
                <button className="primary" onClick={() => runAction(recommendedAction.id)}>Run this action</button>
              </div>
            ) : (
              <EmptyState title="No action selected" body="Refresh the brain state or create a task first." />
            )}
            {workflow?.checks_summary_preview ? (
              <div className="miniOutput">
                <h3>Checks preview</h3>
                <pre>{workflow.checks_summary_preview}</pre>
              </div>
            ) : null}
          </aside>
        </section>
      ) : null}

      {activeTab === "setup" ? (
        <section className="contentGrid two">
          <div className="panel">
            <h2>Connect project</h2>
            <p>Add a local project to RepoDesk and make it active.</p>
            <label>
              Name
              <input value={projectName} onChange={(event) => setProjectName(event.target.value)} placeholder="repodesk" />
            </label>
            <label>
              Path
              <input value={projectPath} onChange={(event) => setProjectPath(event.target.value)} placeholder="/Users/mykyta/Documents/projects/repodesk" />
            </label>
            <label>
              Type
              <input value={projectType} onChange={(event) => setProjectType(event.target.value)} placeholder="rust-cli" />
            </label>
            <button className="primary" onClick={connectProject}>Add and activate project</button>
          </div>
          <div className="panel">
            <h2>Project and task</h2>
            <label>
              Activate project by name
              <input value={switchProjectName} onChange={(event) => setSwitchProjectName(event.target.value)} placeholder="repodesk" />
            </label>
            <button onClick={activateProject}>Use project</button>
            <label>
              New task title
              <input value={taskTitle} onChange={(event) => setTaskTitle(event.target.value)} placeholder="Improve desktop UI feedback" />
            </label>
            <button onClick={createTask}>Create task</button>
            <OutputBlock title="Active project" result={workflow?.project_info} />
            <OutputBlock title="Active task" result={workflow?.task_status} />
          </div>
        </section>
      ) : null}

      {activeTab === "ai" ? (
        <section className="contentGrid two">
          <div className="panel largePanel">
            <div className="panelTitleRow">
              <div>
                <h2>AI discovery</h2>
                <p>Passive scan of installed tools, known apps, and localhost endpoints.</p>
              </div>
              <button className="primary" onClick={runAiScan}>Scan again</button>
            </div>
            {!aiReport ? <EmptyState title="Not scanned yet" body="Run AI scan to see what RepoDesk can detect on this machine." /> : null}
            {aiReport ? (
              <div className="discoveryGrid">
                <div>
                  <h3>Found tools</h3>
                  {foundTools.length === 0 ? <p>No tools detected yet.</p> : foundTools.map((tool) => <DiscoveryTool key={tool.id} tool={tool} />)}
                </div>
                <div>
                  <h3>Missing / unavailable</h3>
                  {missingTools.length === 0 ? <p>No missing tools reported.</p> : missingTools.map((tool) => <DiscoveryTool key={tool.id} tool={tool} />)}
                </div>
                <div>
                  <h3>Local endpoints</h3>
                  {aiReport.endpoints?.map((endpoint) => <DiscoveryEndpoint key={endpoint.id} endpoint={endpoint} />) ?? <p>No endpoints scanned.</p>}
                </div>
              </div>
            ) : null}
          </div>
          <aside className="panel">
            <h2>AI routing notes</h2>
            {(aiReport?.recommendations ?? []).map((item, index) => <p key={index} className="note">{item}</p>)}
            {(aiReport?.warnings ?? []).map((item, index) => <p key={index} className="warningNote">{item}</p>)}
            {aiReport?.report_path ? <code>{aiReport.report_path}</code> : null}
          </aside>
        </section>
      ) : null}

      {activeTab === "artifacts" ? (
        <section className="contentGrid two">
          <div className="panel artifactList">
            <h2>Artifacts</h2>
            {artifactKinds.map((kind) => (
              <button key={kind.kind} className={selectedArtifact.kind === kind.kind ? "selected" : ""} onClick={() => loadArtifact(kind)}>
                <strong>{kind.label}</strong>
                <span>{kind.hint}</span>
              </button>
            ))}
          </div>
          <div className="panel largePanel">
            <div className="panelTitleRow">
              <div>
                <h2>{artifact?.title ?? selectedArtifact.label}</h2>
                <p>{artifact?.path ?? selectedArtifact.hint}</p>
              </div>
              <StatusBadge status={artifact?.exists ? "exists" : "missing"} />
            </div>
            {artifact?.exists ? <pre className="artifactContent">{artifact.content}</pre> : <EmptyState title="Artifact missing" body="Run the related workflow action first." />}
          </div>
        </section>
      ) : null}

      {activeTab === "actions" ? (
        <section className="contentGrid three">
          {actions.map((action) => (
            <article className="panel actionCard" key={action.id}>
              <div className="panelTitleRow">
                <h3>{actionLabel(action)}</h3>
                <StatusBadge status={action.risk ?? "bounded"} />
              </div>
              <p>{action.description ?? "No description."}</p>
              {action.command_preview ? <code>{action.command_preview}</code> : null}
              <button onClick={() => runAction(action.id)}>Run</button>
            </article>
          ))}
        </section>
      ) : null}

      {activeTab === "history" ? (
        <section className="contentGrid two">
          <div className="panel largePanel">
            <h2>Action history</h2>
            {history.length === 0 ? <EmptyState title="No history yet" body="Run an action to create a local receipt." /> : history.map((item, index) => (
              <article className="historyItem" key={`${item.id ?? item.action_id ?? index}-${index}`}>
                <div>
                  <strong>{item.title ?? item.action_id ?? item.id ?? "action"}</strong>
                  <p>{item.message ?? item.error ?? resultText(item.result)}</p>
                </div>
                <StatusBadge status={item.status ?? (commandWasOk(item.result) ? "success" : "warning")} />
              </article>
            ))}
          </div>
          <div className="panel">
            <h2>Last result</h2>
            {lastResult ? <pre>{previewPayload(lastResult)}</pre> : <EmptyState title="No result selected" body="Run a workflow action first." />}
          </div>
        </section>
      ) : null}

      {activeTab === "debug" ? (
        <section className="contentGrid two">
          <div className="panel largePanel">
            <div className="panelTitleRow">
              <div>
                <h2>Debug console</h2>
                <p>Every Tauri command call is recorded here with duration and preview.</p>
              </div>
              <button onClick={() => setDebugEvents([])}>Clear</button>
            </div>
            {debugEvents.length === 0 ? <EmptyState title="No debug events" body="Run refresh, AI scan, or an action." /> : debugEvents.map((event) => (
              <article className={`debugEvent ${event.status}`} key={event.id}>
                <div className="debugHeader">
                  <strong>{event.command}</strong>
                  <span>{event.timestamp} · {formatDuration(event.durationMs)}</span>
                  <StatusBadge status={event.status} />
                </div>
                {event.args ? <pre>{previewPayload(event.args)}</pre> : null}
                {event.error ? <pre className="errorPre">{event.error}</pre> : null}
                {event.preview ? <pre>{event.preview}</pre> : null}
              </article>
            ))}
          </div>
          <div className="panel">
            <h2>Where to debug</h2>
            <p>1. This Debug tab shows frontend ↔ Tauri invocations.</p>
            <p>2. The terminal running <code>./scripts/dev-desktop.sh</code> shows Rust/Tauri errors.</p>
            <p>3. Artifacts tab shows generated context, prompts, and checks summaries.</p>
          </div>
        </section>
      ) : null}

      {activeTab === "raw" ? (
        <section className="panel largePanel">
          <h2>Raw snapshot</h2>
          <pre>{previewPayload({ workflow, snapshot, aiReport, actions, history })}</pre>
        </section>
      ) : null}
    </main>
  );
}

function DiscoveryTool({ tool }: { tool: AiToolProbe }) {
  return (
    <article className="discoveryItem">
      <div>
        <strong>{tool.name}</strong>
        <p>{tool.executable_path ?? tool.app_path ?? tool.detection ?? "No path captured"}</p>
        {tool.notes?.length ? <small>{tool.notes.join(" · ")}</small> : null}
      </div>
      <StatusBadge status={tool.status} />
    </article>
  );
}

function DiscoveryEndpoint({ endpoint }: { endpoint: AiEndpointProbe }) {
  return (
    <article className="discoveryItem">
      <div>
        <strong>{endpoint.name}</strong>
        <p>{endpoint.url}</p>
        {endpoint.notes?.length ? <small>{endpoint.notes.join(" · ")}</small> : null}
      </div>
      <StatusBadge status={endpoint.status} />
    </article>
  );
}
