import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type InvokeState<T> = {
  data: T | null;
  error: string | null;
  loading: boolean;
};

type DesktopAction = {
  id: string;
  title?: string;
  label?: string;
  description?: string;
  risk?: string;
  category?: string;
};

type ActionRunResult = {
  action_id?: string;
  status?: string;
  message?: string;
  started_at?: string;
  finished_at?: string;
  duration_ms?: number;
  error?: string;
};

type AiToolProbe = {
  id: string;
  name: string;
  category: string;
  status: string;
  detection: string;
  executable_path?: string | null;
  app_path?: string | null;
  local_only: boolean;
  requires_paid_account: boolean;
  risk_level: string;
  notes: string[];
};

type AiEndpointProbe = {
  id: string;
  name: string;
  url: string;
  status: string;
  local_only: boolean;
  notes: string[];
};

type AiDiscoveryReport = {
  generated_at: string;
  host_os: string;
  tools: AiToolProbe[];
  endpoints: AiEndpointProbe[];
  recommendations: string[];
  warnings: string[];
  report_path?: string | null;
};

type Snapshot = Record<string, unknown>;

type TabKey = "workflow" | "ai" | "actions" | "security" | "runtime" | "history" | "raw";

const tabs: { key: TabKey; label: string }[] = [
  { key: "workflow", label: "Workflow" },
  { key: "ai", label: "AI Discovery" },
  { key: "actions", label: "Actions" },
  { key: "security", label: "Security" },
  { key: "runtime", label: "Runtime" },
  { key: "history", label: "History" },
  { key: "raw", label: "Raw" },
];

function emptyState<T>(): InvokeState<T> {
  return { data: null, error: null, loading: false };
}

async function safeInvoke<T>(command: string, args?: Record<string, unknown>): Promise<InvokeState<T>> {
  try {
    const data = await invoke<T>(command, args);
    return { data, error: null, loading: false };
  } catch (error) {
    return { data: null, error: String(error), loading: false };
  }
}

function labelForAction(action: DesktopAction): string {
  return action.title ?? action.label ?? action.id;
}

function getSnapshotText(snapshot: Snapshot | null, keys: string[]): string {
  if (!snapshot) return "unknown";
  for (const key of keys) {
    const value = snapshot[key];
    if (typeof value === "string") return value;
    if (typeof value === "number" || typeof value === "boolean") return String(value);
  }
  return "unknown";
}

function hasTruthLike(snapshot: Snapshot | null, keys: string[]): boolean {
  if (!snapshot) return false;
  return keys.some((key) => {
    const value = snapshot[key];
    if (typeof value === "boolean") return value;
    if (typeof value === "string") return value.length > 0 && value !== "unknown" && value !== "false";
    if (typeof value === "number") return value > 0;
    if (Array.isArray(value)) return value.length > 0;
    return Boolean(value);
  });
}

function App() {
  const [activeTab, setActiveTab] = useState<TabKey>("workflow");
  const [snapshot, setSnapshot] = useState<InvokeState<Snapshot>>(emptyState());
  const [actions, setActions] = useState<InvokeState<DesktopAction[]>>(emptyState());
  const [history, setHistory] = useState<InvokeState<ActionRunResult[]>>(emptyState());
  const [aiReport, setAiReport] = useState<InvokeState<AiDiscoveryReport>>(emptyState());
  const [lastRun, setLastRun] = useState<ActionRunResult | null>(null);

  async function refreshAll() {
    setSnapshot((state) => ({ ...state, loading: true }));
    setActions((state) => ({ ...state, loading: true }));
    setHistory((state) => ({ ...state, loading: true }));

    const [snapshotResult, actionsResult, historyResult] = await Promise.all([
      safeInvoke<Snapshot>("desktop_snapshot"),
      safeInvoke<DesktopAction[]>("desktop_actions"),
      safeInvoke<ActionRunResult[]>("action_history"),
    ]);

    setSnapshot(snapshotResult);
    setActions(actionsResult);
    setHistory(historyResult);
  }

  async function scanAiSystems() {
    setAiReport((state) => ({ ...state, loading: true }));
    const result = await safeInvoke<AiDiscoveryReport>("ai_discovery_scan");
    setAiReport(result);
  }

  async function runAction(actionId: string) {
    const result = await safeInvoke<ActionRunResult>("run_desktop_action", { actionId });
    if (result.data) setLastRun(result.data);
    await refreshAll();
  }

  useEffect(() => {
    void refreshAll();
  }, []);

  const recommendedAction = useMemo(() => {
    const list = actions.data ?? [];
    const priority = [
      "workflow_next",
      "build_context",
      "build_smart_context",
      "safety_scan",
      "generate_prompts",
      "run_checks",
      "judge_codex",
    ];
    for (const id of priority) {
      const found = list.find((action) => action.id === id);
      if (found) return found;
    }
    return list[0] ?? null;
  }, [actions.data]);

  const workflowSteps = [
    {
      title: "Project",
      done: hasTruthLike(snapshot.data, ["active_project", "project", "project_name"]),
      detail: getSnapshotText(snapshot.data, ["active_project", "project", "project_name"]),
    },
    {
      title: "Task",
      done: hasTruthLike(snapshot.data, ["active_task", "task", "task_id"]),
      detail: getSnapshotText(snapshot.data, ["active_task", "task", "task_id"]),
    },
    {
      title: "Context",
      done: hasTruthLike(snapshot.data, ["context_exists", "has_context", "context_ready"]),
      detail: "Build bounded context before any paid agent.",
    },
    {
      title: "Smart Context",
      done: hasTruthLike(snapshot.data, ["smart_context_exists", "has_smart_context", "smart_context_ready"]),
      detail: "Use smart context to avoid wasting tokens.",
    },
    {
      title: "Safety",
      done: hasTruthLike(snapshot.data, ["safety_ready", "safety_scan_exists", "security_ready"]),
      detail: "Scan for secrets and risky context before routing to AI.",
    },
    {
      title: "Prompts",
      done: hasTruthLike(snapshot.data, ["prompts_exist", "has_prompts", "prompt_count"]),
      detail: "Generate task-specific prompts for the right runtime.",
    },
    {
      title: "Checks",
      done: hasTruthLike(snapshot.data, ["checks_summary_exists", "has_checks_summary", "checks_ready"]),
      detail: "Run configured checks and summarize failures.",
    },
  ];

  return (
    <main className="shell">
      <header className="hero">
        <div>
          <p className="eyebrow">RepoDesk Control Brain</p>
          <h1>Local-first AI workflow cockpit</h1>
          <p className="heroText">
            RepoDesk should decide what is safe, what is expensive, which AI runtime fits the job,
            and what the next useful step is before any agent touches your repository.
          </p>
        </div>
        <div className="heroActions">
          <button className="secondary" onClick={() => void refreshAll()}>Refresh brain</button>
          <button className="secondary" onClick={() => void scanAiSystems()}>Scan AI systems</button>
          <button
            className="primary"
            disabled={!recommendedAction}
            onClick={() => recommendedAction && void runAction(recommendedAction.id)}
          >
            Do next safe step
          </button>
        </div>
      </header>

      <nav className="tabs">
        {tabs.map((tab) => (
          <button
            key={tab.key}
            className={activeTab === tab.key ? "active" : ""}
            onClick={() => setActiveTab(tab.key)}
          >
            {tab.label}
          </button>
        ))}
      </nav>

      {lastRun && (
        <section className="notice">
          <strong>Last action:</strong> {lastRun.action_id ?? "action"} — {lastRun.status ?? "finished"}
          {lastRun.message ? <span> · {lastRun.message}</span> : null}
          {lastRun.error ? <span className="danger"> · {lastRun.error}</span> : null}
        </section>
      )}

      {activeTab === "workflow" && (
        <section className="grid two">
          <article className="card span2">
            <div className="cardHeader">
              <div>
                <p className="eyebrow">Product workflow</p>
                <h2>What should happen next?</h2>
              </div>
              {recommendedAction ? <span className="pill">Next: {labelForAction(recommendedAction)}</span> : null}
            </div>
            <div className="timeline">
              {workflowSteps.map((step, index) => (
                <div className={`step ${step.done ? "done" : "pending"}`} key={step.title}>
                  <div className="stepIndex">{step.done ? "✓" : index + 1}</div>
                  <div>
                    <h3>{step.title}</h3>
                    <p>{step.detail}</p>
                  </div>
                </div>
              ))}
            </div>
          </article>
          <SnapshotCard snapshot={snapshot} />
          <article className="card">
            <p className="eyebrow">Decision rule</p>
            <h2>Never pay for chaos</h2>
            <p>
              The desktop app must not send raw repositories to paid agents. First build context,
              reduce it, scan it, judge it, and only then route to Codex/ChatGPT/Gemini if needed.
            </p>
          </article>
        </section>
      )}

      {activeTab === "ai" && (
        <AiDiscovery report={aiReport} onScan={scanAiSystems} />
      )}

      {activeTab === "actions" && (
        <section className="grid two">
          {(actions.data ?? []).map((action) => (
            <article className="card" key={action.id}>
              <div className="cardHeader">
                <div>
                  <p className="eyebrow">{action.category ?? "action"}</p>
                  <h2>{labelForAction(action)}</h2>
                </div>
                <span className={`pill ${action.risk === "high" ? "warn" : ""}`}>{action.risk ?? "bounded"}</span>
              </div>
              <p>{action.description ?? "Bounded RepoDesk action."}</p>
              <button className="secondary" onClick={() => void runAction(action.id)}>Run guarded action</button>
            </article>
          ))}
          {actions.error ? <ErrorCard title="Actions unavailable" error={actions.error} /> : null}
        </section>
      )}

      {activeTab === "security" && (
        <section className="grid two">
          <article className="card span2">
            <p className="eyebrow">Security model</p>
            <h2>Desktop UI is not a shell</h2>
            <p>
              UI actions are whitelisted Tauri commands. No arbitrary command input, no secret reading,
              no full repository exfiltration, no paid-agent routing without context and judge checks.
            </p>
          </article>
          <article className="card">
            <h2>Allowed</h2>
            <ul>
              <li>Read dashboard state</li>
              <li>Build bounded context</li>
              <li>Scan known local AI tools passively</li>
              <li>Run configured checks through RepoDesk action layer</li>
            </ul>
          </article>
          <article className="card">
            <h2>Blocked by design</h2>
            <ul>
              <li>Unrestricted shell from UI</li>
              <li>Secrets in prompts</li>
              <li>External AI with raw repository context</li>
              <li>Agent patching without review receipts</li>
            </ul>
          </article>
        </section>
      )}

      {activeTab === "runtime" && (
        <section className="grid two">
          <article className="card span2">
            <p className="eyebrow">Runtime routing</p>
            <h2>AI should be treated as modules</h2>
            <p>
              Local runtimes should handle compression and private context. Paid agents should only receive
              reduced task context and explicit patch instructions. Editors and CLI agents are peripherals,
              not the brain itself.
            </p>
          </article>
          <AiMiniSummary report={aiReport.data} />
        </section>
      )}

      {activeTab === "history" && (
        <section className="grid two">
          {(history.data ?? []).map((item, index) => (
            <article className="card" key={`${item.action_id ?? "action"}-${index}`}>
              <p className="eyebrow">{item.started_at ?? "recent"}</p>
              <h2>{item.action_id ?? "Action"}</h2>
              <p>Status: {item.status ?? "unknown"}</p>
              {item.message ? <p>{item.message}</p> : null}
              {item.error ? <p className="danger">{item.error}</p> : null}
            </article>
          ))}
          {history.error ? <ErrorCard title="History unavailable" error={history.error} /> : null}
        </section>
      )}

      {activeTab === "raw" && (
        <section className="card">
          <p className="eyebrow">Debug snapshot</p>
          <pre>{JSON.stringify({ snapshot: snapshot.data, ai: aiReport.data, actions: actions.data, history: history.data }, null, 2)}</pre>
        </section>
      )}
    </main>
  );
}

function SnapshotCard({ snapshot }: { snapshot: InvokeState<Snapshot> }) {
  if (snapshot.error) return <ErrorCard title="Snapshot unavailable" error={snapshot.error} />;

  return (
    <article className="card">
      <p className="eyebrow">Brain snapshot</p>
      <h2>Current state</h2>
      <div className="kv">
        <span>Project</span>
        <strong>{getSnapshotText(snapshot.data, ["active_project", "project", "project_name"])}</strong>
        <span>Task</span>
        <strong>{getSnapshotText(snapshot.data, ["active_task", "task", "task_id"])}</strong>
        <span>Budget</span>
        <strong>{getSnapshotText(snapshot.data, ["budget_level", "budget", "token_budget"])} </strong>
        <span>Next</span>
        <strong>{getSnapshotText(snapshot.data, ["next_action", "recommended_next_action"])}</strong>
      </div>
    </article>
  );
}

function AiDiscovery({ report, onScan }: { report: InvokeState<AiDiscoveryReport>; onScan: () => Promise<void> }) {
  const tools = report.data?.tools ?? [];
  const endpoints = report.data?.endpoints ?? [];
  const availableTools = tools.filter((tool) => tool.status === "available");

  return (
    <section className="grid two">
      <article className="card span2">
        <div className="cardHeader">
          <div>
            <p className="eyebrow">AI discovery</p>
            <h2>Scan installed AI systems and runtimes</h2>
          </div>
          <button className="primary" onClick={() => void onScan()} disabled={report.loading}>
            {report.loading ? "Scanning..." : "Scan now"}
          </button>
        </div>
        <p>
          Passive scan only: PATH lookup, known desktop app paths, and localhost ports. It does not execute agents,
          read secrets, or contact external AI providers.
        </p>
        {report.error ? <p className="danger">{report.error}</p> : null}
        {report.data?.report_path ? <p className="muted">Saved: {report.data.report_path}</p> : null}
      </article>

      <article className="card">
        <p className="eyebrow">Available tools</p>
        <h2>{availableTools.length} detected</h2>
        <div className="toolList">
          {availableTools.map((tool) => <ToolRow key={tool.id} tool={tool} />)}
          {availableTools.length === 0 ? <p>No AI tools detected yet. Start Ollama or install a local runtime.</p> : null}
        </div>
      </article>

      <article className="card">
        <p className="eyebrow">Local endpoints</p>
        <h2>Runtime ports</h2>
        <div className="toolList">
          {endpoints.map((endpoint) => (
            <div className="toolRow" key={endpoint.id}>
              <div>
                <strong>{endpoint.name}</strong>
                <p>{endpoint.url}</p>
              </div>
              <span className={`pill ${endpoint.status === "available" ? "ok" : ""}`}>{endpoint.status}</span>
            </div>
          ))}
        </div>
      </article>

      <article className="card">
        <p className="eyebrow">Recommendations</p>
        <h2>Routing hints</h2>
        <ul>
          {(report.data?.recommendations ?? []).map((item) => <li key={item}>{item}</li>)}
        </ul>
      </article>

      <article className="card">
        <p className="eyebrow">Warnings</p>
        <h2>Guardrails</h2>
        <ul>
          {(report.data?.warnings ?? []).map((item) => <li key={item}>{item}</li>)}
        </ul>
      </article>
    </section>
  );
}

function AiMiniSummary({ report }: { report: AiDiscoveryReport | null }) {
  const available = report?.tools.filter((tool) => tool.status === "available") ?? [];
  return (
    <article className="card span2">
      <div className="cardHeader">
        <div>
          <p className="eyebrow">Detected modules</p>
          <h2>{available.length} available AI/peripheral tools</h2>
        </div>
        <span className="pill">passive scan</span>
      </div>
      <div className="toolList compact">
        {available.map((tool) => <ToolRow key={tool.id} tool={tool} />)}
        {available.length === 0 ? <p>Run AI Discovery to populate runtime status.</p> : null}
      </div>
    </article>
  );
}

function ToolRow({ tool }: { tool: AiToolProbe }) {
  return (
    <div className="toolRow">
      <div>
        <strong>{tool.name}</strong>
        <p>{tool.executable_path ?? tool.app_path ?? tool.detection}</p>
      </div>
      <span className={`pill ${tool.status === "available" ? "ok" : ""}`}>{tool.status}</span>
    </div>
  );
}

function ErrorCard({ title, error }: { title: string; error: string }) {
  return (
    <article className="card errorCard">
      <p className="eyebrow">Error</p>
      <h2>{title}</h2>
      <p>{error}</p>
    </article>
  );
}

export default App;
