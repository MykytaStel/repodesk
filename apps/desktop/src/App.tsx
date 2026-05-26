import { useEffect, useMemo, useState } from "react";
import {
  ActionRunResult,
  CommandResult,
  DesktopAction,
  DesktopSnapshot,
  explainAction,
  loadActionHistory,
  loadSnapshot,
  projectAdd,
  projectInfo,
  projectList,
  projectUse,
  runAction,
  taskNew,
  taskShow,
  taskStatus,
} from "./api";
import { PRODUCT_AREAS, WORKFLOW_STEPS } from "./product";
import "./styles.css";

type Tab = "dashboard" | "management" | "workflow" | "actions" | "security" | "runtime" | "history" | "raw";

function StatusPill({ ok, label }: { ok: boolean; label?: string }) {
  return <span className={ok ? "pill pill-ok" : "pill pill-warn"}>{label ?? (ok ? "OK" : "Needs attention")}</span>;
}

function RiskPill({ risk }: { risk: string }) {
  const normalized = risk.toLowerCase();
  const cls = normalized.includes("safe")
    ? "pill-ok"
    : normalized.includes("blocked")
      ? "pill-block"
      : normalized.includes("expensive")
        ? "pill-expensive"
        : "pill-warn";
  return <span className={`pill ${cls}`}>{risk}</span>;
}

function OutputBlock({ title, result }: { title: string; result?: CommandResult }) {
  if (!result) return null;

  return (
    <section className="panel output-panel">
      <div className="panel-head">
        <div>
          <h3>{title}</h3>
          <p>{result.command}</p>
        </div>
        <StatusPill ok={result.ok} />
      </div>
      {result.stdout ? <pre>{result.stdout}</pre> : null}
      {result.stderr ? <pre className="stderr">{result.stderr}</pre> : null}
      {!result.stdout && !result.stderr ? <p className="muted">No output yet.</p> : null}
    </section>
  );
}

function Header({ snapshot, onRefresh }: { snapshot: DesktopSnapshot | null; onRefresh: () => void }) {
  return (
    <header className="hero">
      <div>
        <p className="eyebrow">RepoDesk Desktop</p>
        <h1>Control brain for AI-assisted development</h1>
        <p className="hero-text">
          Manage projects and tasks, build bounded context, judge AI agents, run safe actions, and keep the workflow explainable.
        </p>
      </div>
      <div className="hero-card">
        <span className="label">Workspace</span>
        <strong>{snapshot?.workspace_root ?? "Loading..."}</strong>
        <span className="label">Mode</span>
        <strong>{snapshot?.mode ?? "desktop"}</strong>
        <button onClick={onRefresh}>Refresh state</button>
      </div>
    </header>
  );
}

function Dashboard({ snapshot }: { snapshot: DesktopSnapshot }) {
  const signals = [
    { title: "Dashboard", result: snapshot.dashboard },
    { title: "Project", result: snapshot.project_info },
    { title: "Task", result: snapshot.task_status },
    { title: "Doctor", result: snapshot.doctor },
    { title: "Security", result: snapshot.security },
    { title: "Git", result: snapshot.git },
  ];

  return (
    <div className="stack">
      <section className="grid areas">
        {PRODUCT_AREAS.map((area) => (
          <article className="panel area-card" key={area.id}>
            <span className="label">{area.signal}</span>
            <h3>{area.title}</h3>
            <h4>{area.subtitle}</h4>
            <p>{area.description}</p>
          </article>
        ))}
      </section>

      <section className="grid status-grid">
        {signals.map((signal) => (
          <article className="panel compact" key={signal.title}>
            <div className="panel-head">
              <h3>{signal.title}</h3>
              <StatusPill ok={signal.result.ok} />
            </div>
            <pre>{signal.result.stdout || signal.result.stderr || "No output"}</pre>
          </article>
        ))}
      </section>
    </div>
  );
}

function Management({ snapshot, onRefresh }: { snapshot: DesktopSnapshot; onRefresh: () => void }) {
  const [projectName, setProjectName] = useState("");
  const [projectPath, setProjectPath] = useState("");
  const [projectType, setProjectType] = useState("rust-cli");
  const [mainLanguage, setMainLanguage] = useState("rust");
  const [switchProjectName, setSwitchProjectName] = useState("");
  const [taskTitle, setTaskTitle] = useState("");
  const [result, setResult] = useState<CommandResult | null>(null);
  const [busy, setBusy] = useState(false);

  async function runManagedAction(action: () => Promise<CommandResult>) {
    setBusy(true);
    try {
      const next = await action();
      setResult(next);
      await onRefresh();
    } catch (error) {
      setResult({
        ok: false,
        command: "desktop management action",
        stdout: "",
        stderr: error instanceof Error ? error.message : String(error),
        exit_code: null,
      });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="stack">
      <section className="panel callout">
        <h2>Project and task management</h2>
        <p>
          This is the entry point for the real product workflow: select the active project, create a task, then build context and run guarded actions.
        </p>
      </section>

      <section className="grid two">
        <OutputBlock title="Active project" result={snapshot.project_info} />
        <OutputBlock title="Active task" result={snapshot.task_show} />
      </section>

      <section className="grid two">
        <form
          className="panel form-panel"
          onSubmit={(event) => {
            event.preventDefault();
            void runManagedAction(() =>
              projectAdd({
                name: projectName,
                path: projectPath,
                project_type: projectType,
                main_language: mainLanguage || null,
              }),
            );
          }}
        >
          <h3>Add project</h3>
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
          <label>
            Main language
            <input value={mainLanguage} onChange={(event) => setMainLanguage(event.target.value)} placeholder="rust" />
          </label>
          <button disabled={busy}>Add project</button>
        </form>

        <form
          className="panel form-panel"
          onSubmit={(event) => {
            event.preventDefault();
            void runManagedAction(() => projectUse(switchProjectName));
          }}
        >
          <h3>Switch project</h3>
          <label>
            Project name
            <input value={switchProjectName} onChange={(event) => setSwitchProjectName(event.target.value)} placeholder="repopilot" />
          </label>
          <button disabled={busy}>Use project</button>
          <button type="button" className="secondary" onClick={() => void runManagedAction(projectList)} disabled={busy}>
            List projects
          </button>
          <button type="button" className="secondary" onClick={() => void runManagedAction(projectInfo)} disabled={busy}>
            Refresh project info
          </button>
        </form>
      </section>

      <section className="grid two">
        <form
          className="panel form-panel"
          onSubmit={(event) => {
            event.preventDefault();
            void runManagedAction(() => taskNew(taskTitle));
          }}
        >
          <h3>Create task</h3>
          <label>
            Task title
            <input value={taskTitle} onChange={(event) => setTaskTitle(event.target.value)} placeholder="Build desktop management UI" />
          </label>
          <button disabled={busy}>Create task</button>
          <button type="button" className="secondary" onClick={() => void runManagedAction(taskStatus)} disabled={busy}>
            Task status
          </button>
          <button type="button" className="secondary" onClick={() => void runManagedAction(taskShow)} disabled={busy}>
            Show task
          </button>
        </form>
        <OutputBlock title="Management result" result={result ?? snapshot.project_list} />
      </section>
    </div>
  );
}

function Workflow() {
  return (
    <div className="timeline">
      {WORKFLOW_STEPS.map((step) => (
        <article className="panel timeline-item" key={step.id}>
          <h3>{step.title}</h3>
          <p>{step.body}</p>
        </article>
      ))}
    </div>
  );
}

function Actions({ actions, onRun, busy }: { actions: DesktopAction[]; onRun: (id: string) => void; busy: string | null }) {
  const grouped = useMemo(() => {
    return actions.reduce<Record<string, DesktopAction[]>>((acc, action) => {
      acc[action.category] ??= [];
      acc[action.category].push(action);
      return acc;
    }, {});
  }, [actions]);

  return (
    <div className="stack">
      {Object.entries(grouped).map(([category, items]) => (
        <section className="panel" key={category}>
          <h2>{category}</h2>
          <div className="action-grid">
            {items.map((action) => (
              <article className="action-card" key={action.id}>
                <div className="panel-head">
                  <h3>{action.title}</h3>
                  <RiskPill risk={action.risk} />
                </div>
                <p>{action.description}</p>
                <code>{action.command_preview}</code>
                <button onClick={() => onRun(action.id)} disabled={busy !== null}>
                  {busy === action.id ? "Running..." : "Run bounded action"}
                </button>
              </article>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}

function History({ history, lastRun }: { history: ActionRunResult[]; lastRun: ActionRunResult | null }) {
  const items = lastRun ? [lastRun, ...history.filter((item) => item.started_at_ms !== lastRun.started_at_ms)] : history;

  if (items.length === 0) {
    return <section className="panel"><h2>No action history yet</h2><p>Run a bounded action to create a receipt.</p></section>;
  }

  return (
    <div className="stack">
      {items.map((item) => (
        <section className="panel output-panel" key={`${item.id}-${item.started_at_ms}`}>
          <div className="panel-head">
            <div>
              <h3>{item.title}</h3>
              <p>{item.result.command}</p>
            </div>
            <StatusPill ok={item.result.ok} />
          </div>
          <RiskPill risk={item.risk} />
          {item.result.stdout ? <pre>{item.result.stdout}</pre> : null}
          {item.result.stderr ? <pre className="stderr">{item.result.stderr}</pre> : null}
        </section>
      ))}
    </div>
  );
}

function App() {
  const [tab, setTab] = useState<Tab>("dashboard");
  const [snapshot, setSnapshot] = useState<DesktopSnapshot | null>(null);
  const [history, setHistory] = useState<ActionRunResult[]>([]);
  const [lastRun, setLastRun] = useState<ActionRunResult | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    setError(null);
    try {
      const [nextSnapshot, nextHistory] = await Promise.all([loadSnapshot(), loadActionHistory()]);
      setSnapshot(nextSnapshot);
      setHistory(nextHistory);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function handleRun(actionId: string) {
    setBusyAction(actionId);
    setError(null);
    try {
      const explanation = await explainAction(actionId);
      console.info(explanation);
      const result = await runAction(actionId);
      setLastRun(result);
      setTab("history");
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyAction(null);
    }
  }

  if (!snapshot) {
    return (
      <main className="app-shell">
        <section className="panel loading-card">
          <h1>Loading RepoDesk...</h1>
          {error ? <pre className="stderr">{error}</pre> : <p>Reading local brain state.</p>}
          <button onClick={() => void refresh()}>Retry</button>
        </section>
      </main>
    );
  }

  const tabs: { id: Tab; label: string }[] = [
    { id: "dashboard", label: "Dashboard" },
    { id: "management", label: "Management" },
    { id: "workflow", label: "Workflow" },
    { id: "actions", label: "Actions" },
    { id: "security", label: "Security" },
    { id: "runtime", label: "Runtime" },
    { id: "history", label: "History" },
    { id: "raw", label: "Raw" },
  ];

  return (
    <main className="app-shell">
      <Header snapshot={snapshot} onRefresh={() => void refresh()} />

      <nav className="tabs">
        {tabs.map((item) => (
          <button key={item.id} className={tab === item.id ? "active" : ""} onClick={() => setTab(item.id)}>
            {item.label}
          </button>
        ))}
      </nav>

      {error ? <pre className="stderr global-error">{error}</pre> : null}

      {tab === "dashboard" ? <Dashboard snapshot={snapshot} /> : null}
      {tab === "management" ? <Management snapshot={snapshot} onRefresh={refresh} /> : null}
      {tab === "workflow" ? <Workflow /> : null}
      {tab === "actions" ? <Actions actions={snapshot.actions} onRun={(id) => void handleRun(id)} busy={busyAction} /> : null}
      {tab === "security" ? (
        <div className="stack"><OutputBlock title="Security audit" result={snapshot.security} /><OutputBlock title="Workflow doctor" result={snapshot.doctor} /></div>
      ) : null}
      {tab === "runtime" ? (
        <div className="stack"><OutputBlock title="Runtime providers" result={snapshot.runtime} /><OutputBlock title="Git audit" result={snapshot.git} /></div>
      ) : null}
      {tab === "history" ? <History history={history} lastRun={lastRun} /> : null}
      {tab === "raw" ? <pre className="raw-json">{JSON.stringify(snapshot, null, 2)}</pre> : null}
    </main>
  );
}

export default App;
