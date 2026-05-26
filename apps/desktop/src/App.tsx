import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ActionRunResult,
  ArtifactContent,
  CommandResult,
  DesktopAction,
  DesktopSnapshot,
  ProductWorkflowState,
  explainAction,
  getActionHistory,
  getSnapshot,
  getWorkflowState,
  projectAdd,
  projectList,
  projectUse,
  readArtifact,
  runDesktopAction,
  runNextSafeStep,
  taskNew,
} from "./api";
import { artifactKinds, formatBytes, productPrinciples, statusLabel, summarizeCommand } from "./product";
import "./styles.css";

type TabId = "workflow" | "management" | "artifacts" | "actions" | "security" | "runtime" | "history" | "raw";

const tabs: Array<{ id: TabId; label: string }> = [
  { id: "workflow", label: "Workflow" },
  { id: "management", label: "Management" },
  { id: "artifacts", label: "Artifacts" },
  { id: "actions", label: "Actions" },
  { id: "security", label: "Security" },
  { id: "runtime", label: "Runtime" },
  { id: "history", label: "History" },
  { id: "raw", label: "Raw" },
];

function OutputBlock({ result }: { result?: CommandResult }) {
  if (!result) return <pre className="output muted">No output yet.</pre>;
  return (
    <pre className={result.ok ? "output" : "output outputError"}>
      {result.stdout || result.stderr || "No output"}
    </pre>
  );
}

function StatusPill({ ok, label }: { ok: boolean; label: string }) {
  return <span className={ok ? "pill pillOk" : "pill pillWarn"}>{label}</span>;
}

function StepCard({ step }: { step: ProductWorkflowState["steps"][number] }) {
  return (
    <article className={`stepCard ${step.status}`}>
      <div className="stepHead">
        <span className="stepTitle">{step.title}</span>
        <span className={`stepStatus ${step.status}`}>{statusLabel(step.status)}</span>
      </div>
      <p>{step.description}</p>
      {step.command_preview && <code>{step.command_preview}</code>}
      {step.blocker && <small className="warnText">{step.blocker}</small>}
    </article>
  );
}

function RunResult({ result }: { result?: ActionRunResult | null }) {
  if (!result) return null;
  const duration = Number(result.finished_at_ms - result.started_at_ms);
  return (
    <section className="card resultCard">
      <div className="sectionHeader">
        <div>
          <h3>{result.title}</h3>
          <p>{result.result.command}</p>
        </div>
        <StatusPill ok={result.result.ok} label={result.result.ok ? "Success" : "Failed"} />
      </div>
      <p className="mutedText">Duration: {duration} ms · Risk: {result.risk}</p>
      <OutputBlock result={result.result} />
    </section>
  );
}

function WorkflowView({
  workflow,
  onRunNext,
  running,
}: {
  workflow: ProductWorkflowState | null;
  onRunNext: () => void;
  running: boolean;
}) {
  if (!workflow) {
    return <div className="card">Loading workflow...</div>;
  }

  return (
    <div className="grid gap">
      <section className="heroPanel">
        <div>
          <p className="eyebrow">RepoDesk Control Brain</p>
          <h2>{workflow.primary_cta}</h2>
          <p>
            RepoDesk guides the active project from task → context → safety → prompt → checks → review.
            The desktop UI can only run whitelisted commands.
          </p>
          <div className="healthRow">
            <StatusPill ok={workflow.project_ok} label="Project" />
            <StatusPill ok={workflow.task_ok} label="Task" />
            <StatusPill ok={workflow.smart_context_ok} label="Smart context" />
            <StatusPill ok={workflow.safety_ok} label="Safety" />
            <StatusPill ok={workflow.prompts_ok} label="Prompts" />
            <StatusPill ok={workflow.checks_ok} label="Checks" />
          </div>
        </div>
        <div className="primaryActionBox">
          <span className="mutedText">Recommended action</span>
          <strong>{workflow.recommended_action_title || workflow.primary_cta}</strong>
          <button disabled={running || !workflow.recommended_action_id} onClick={onRunNext}>
            {running ? "Running..." : "Do next safe step"}
          </button>
          {!workflow.recommended_action_id && (
            <small className="warnText">Set project/task first in Management.</small>
          )}
        </div>
      </section>

      <section className="timeline">
        {workflow.steps.map((step) => (
          <StepCard key={step.id} step={step} />
        ))}
      </section>

      <section className="twoCols">
        <div className="card">
          <h3>Brain hint</h3>
          <OutputBlock result={workflow.workflow_hint} />
        </div>
        <div className="card">
          <h3>Security verdict</h3>
          <OutputBlock result={workflow.security_verdict} />
        </div>
      </section>

      {workflow.checks_summary_preview && (
        <section className="card">
          <h3>Latest checks summary</h3>
          <pre className="output">{workflow.checks_summary_preview}</pre>
        </section>
      )}
    </div>
  );
}

function ManagementView({ onRefresh }: { onRefresh: () => void }) {
  const [projectName, setProjectName] = useState("repodesk");
  const [projectPath, setProjectPath] = useState("");
  const [projectType, setProjectType] = useState("rust-desktop");
  const [mainLanguage, setMainLanguage] = useState("rust");
  const [useProjectName, setUseProjectName] = useState("");
  const [taskTitle, setTaskTitle] = useState("Build product workflow MVP");
  const [projectListOutput, setProjectListOutput] = useState<CommandResult | null>(null);
  const [lastResult, setLastResult] = useState<CommandResult | null>(null);
  const [busy, setBusy] = useState(false);

  const run = async (fn: () => Promise<CommandResult>) => {
    setBusy(true);
    try {
      const result = await fn();
      setLastResult(result);
      onRefresh();
    } catch (error) {
      setLastResult({ ok: false, command: "desktop management", stdout: "", stderr: String(error), exit_code: null });
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    projectList().then(setProjectListOutput).catch(() => undefined);
  }, []);

  return (
    <div className="twoCols">
      <section className="card">
        <h3>Add project</h3>
        <label>Project name</label>
        <input value={projectName} onChange={(event) => setProjectName(event.target.value)} />
        <label>Project path</label>
        <input placeholder="/Users/mykyta/Documents/projects/repodesk" value={projectPath} onChange={(event) => setProjectPath(event.target.value)} />
        <label>Project type</label>
        <input value={projectType} onChange={(event) => setProjectType(event.target.value)} />
        <label>Main language</label>
        <input value={mainLanguage} onChange={(event) => setMainLanguage(event.target.value)} />
        <button disabled={busy} onClick={() => run(() => projectAdd({ name: projectName, path: projectPath, project_type: projectType, main_language: mainLanguage }))}>
          Add project
        </button>
      </section>

      <section className="card">
        <h3>Use project / create task</h3>
        <label>Project name</label>
        <input value={useProjectName} onChange={(event) => setUseProjectName(event.target.value)} placeholder="repodesk" />
        <button disabled={busy} onClick={() => run(() => projectUse(useProjectName))}>Use project</button>
        <hr />
        <label>Task title</label>
        <input value={taskTitle} onChange={(event) => setTaskTitle(event.target.value)} />
        <button disabled={busy} onClick={() => run(() => taskNew(taskTitle))}>Create active task</button>
      </section>

      <section className="card">
        <h3>Registered projects</h3>
        <OutputBlock result={projectListOutput || undefined} />
      </section>

      <section className="card">
        <h3>Last management result</h3>
        <OutputBlock result={lastResult || undefined} />
      </section>
    </div>
  );
}

function ArtifactsView({ workflow }: { workflow: ProductWorkflowState | null }) {
  const [selectedKind, setSelectedKind] = useState("prompt_codex");
  const [artifact, setArtifact] = useState<ArtifactContent | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadArtifact = useCallback(async (kind: string) => {
    setSelectedKind(kind);
    setError(null);
    try {
      setArtifact(await readArtifact(kind));
    } catch (err) {
      setArtifact(null);
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    loadArtifact(selectedKind);
  }, [loadArtifact, selectedKind]);

  const copy = async () => {
    if (!artifact?.content) return;
    await navigator.clipboard.writeText(artifact.content);
  };

  return (
    <div className="grid gap">
      <section className="card">
        <div className="sectionHeader">
          <div>
            <h3>Artifacts</h3>
            <p>Read prompts and summaries without opening files manually.</p>
          </div>
          <button disabled={!artifact?.content} onClick={copy}>Copy content</button>
        </div>
        <div className="artifactButtons">
          {artifactKinds.map((item) => {
            const status = workflow?.artifacts.find((artifactStatus) => artifactStatus.kind === item.kind);
            return (
              <button key={item.kind} className={selectedKind === item.kind ? "secondary active" : "secondary"} onClick={() => loadArtifact(item.kind)}>
                {item.label} {status?.exists ? "✓" : ""}
              </button>
            );
          })}
        </div>
        {workflow && (
          <div className="artifactGrid">
            {workflow.artifacts.map((item) => (
              <div key={item.kind} className="artifactMeta">
                <strong>{item.title}</strong>
                <span>{item.exists ? formatBytes(item.size_bytes) : "missing"}</span>
              </div>
            ))}
          </div>
        )}
      </section>

      <section className="card">
        <div className="sectionHeader">
          <div>
            <h3>{artifact?.title || "Artifact"}</h3>
            <p>{artifact?.path || error || "No artifact selected"}</p>
          </div>
          {artifact?.exists ? <StatusPill ok label="Exists" /> : <StatusPill ok={false} label="Missing" />}
        </div>
        {error ? <pre className="output outputError">{error}</pre> : <pre className="output largeOutput">{artifact?.content || "No content yet."}</pre>}
      </section>
    </div>
  );
}

function ActionsView({
  actions,
  onRun,
  running,
}: {
  actions: DesktopAction[];
  onRun: (id: string) => void;
  running: string | null;
}) {
  const [explanation, setExplanation] = useState<string>("");
  const grouped = useMemo(() => {
    return actions.reduce<Record<string, DesktopAction[]>>((acc, action) => {
      acc[action.category] ||= [];
      acc[action.category].push(action);
      return acc;
    }, {});
  }, [actions]);

  return (
    <div className="twoCols">
      <section className="card">
        <h3>Bounded actions</h3>
        {Object.entries(grouped).map(([category, categoryActions]) => (
          <div key={category} className="actionGroup">
            <h4>{category}</h4>
            {categoryActions.map((action) => (
              <div className="actionRow" key={action.id}>
                <div>
                  <strong>{action.title}</strong>
                  <p>{action.description}</p>
                  <code>{action.command_preview}</code>
                </div>
                <div className="actionButtons">
                  <button className="secondary" onClick={async () => setExplanation(await explainAction(action.id))}>Explain</button>
                  <button disabled={Boolean(running)} onClick={() => onRun(action.id)}>
                    {running === action.id ? "Running..." : "Run"}
                  </button>
                </div>
              </div>
            ))}
          </div>
        ))}
      </section>

      <section className="card stickyCard">
        <h3>Action explanation</h3>
        <pre className="output">{explanation || "Pick an action to see why it is allowed and what it can do."}</pre>
      </section>
    </div>
  );
}

function HistoryView({ history }: { history: ActionRunResult[] }) {
  return (
    <section className="card">
      <h3>Action history</h3>
      {history.length === 0 && <p>No desktop actions recorded yet.</p>}
      <div className="historyList">
        {history.map((item, index) => (
          <article key={`${item.started_at_ms}-${index}`} className="historyItem">
            <div className="sectionHeader">
              <div>
                <strong>{item.title}</strong>
                <p>{item.result.command}</p>
              </div>
              <StatusPill ok={item.result.ok} label={item.result.ok ? "OK" : "Failed"} />
            </div>
            <pre className="output compactOutput">{summarizeCommand(item.result)}</pre>
          </article>
        ))}
      </div>
    </section>
  );
}

function App() {
  const [tab, setTab] = useState<TabId>("workflow");
  const [snapshot, setSnapshot] = useState<DesktopSnapshot | null>(null);
  const [workflow, setWorkflow] = useState<ProductWorkflowState | null>(null);
  const [actions, setActions] = useState<DesktopAction[]>([]);
  const [history, setHistory] = useState<ActionRunResult[]>([]);
  const [lastRun, setLastRun] = useState<ActionRunResult | null>(null);
  const [runningAction, setRunningAction] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    const nextSnapshot = await getSnapshot();
    const nextWorkflow = await getWorkflowState();
    const nextHistory = await getActionHistory();
    setSnapshot(nextSnapshot);
    setWorkflow(nextWorkflow);
    setActions(nextSnapshot.actions || []);
    setHistory(nextHistory);
    setLoading(false);
  }, []);

  useEffect(() => {
    refresh().catch((error) => {
      console.error(error);
      setLoading(false);
    });
  }, [refresh]);

  const runAction = async (actionId: string) => {
    setRunningAction(actionId);
    try {
      const result = await runDesktopAction(actionId);
      setLastRun(result);
      await refresh();
    } catch (error) {
      setLastRun({
        id: actionId,
        title: "Action failed before execution",
        risk: "unknown",
        category: "Desktop",
        started_at_ms: Date.now(),
        finished_at_ms: Date.now(),
        result: { ok: false, command: actionId, stdout: "", stderr: String(error), exit_code: null },
      });
    } finally {
      setRunningAction(null);
    }
  };

  const runPrimary = async () => {
    setRunningAction(workflow?.recommended_action_id || "next");
    try {
      const result = await runNextSafeStep();
      setLastRun(result);
      await refresh();
    } catch (error) {
      setLastRun({
        id: "next",
        title: "Primary action failed",
        risk: "unknown",
        category: "Workflow",
        started_at_ms: Date.now(),
        finished_at_ms: Date.now(),
        result: { ok: false, command: "run next safe step", stdout: "", stderr: String(error), exit_code: null },
      });
    } finally {
      setRunningAction(null);
    }
  };

  if (loading) {
    return <main className="appShell"><div className="card">Loading RepoDesk...</div></main>;
  }

  return (
    <main className="appShell">
      <header className="topbar">
        <div>
          <p className="eyebrow">Local AI development cockpit</p>
          <h1>RepoDesk</h1>
        </div>
        <div className="topActions">
          <StatusPill ok={Boolean(workflow?.project_ok)} label="Project" />
          <StatusPill ok={Boolean(workflow?.task_ok)} label="Task" />
          <StatusPill ok={Boolean(workflow?.safety_ok)} label="Safety" />
          <button className="secondary" onClick={refresh}>Refresh</button>
        </div>
      </header>

      <section className="principles">
        {productPrinciples.map((item) => (
          <article key={item.title}>
            <strong>{item.title}</strong>
            <p>{item.body}</p>
          </article>
        ))}
      </section>

      <nav className="tabs">
        {tabs.map((item) => (
          <button key={item.id} className={tab === item.id ? "active" : ""} onClick={() => setTab(item.id)}>
            {item.label}
          </button>
        ))}
      </nav>

      {lastRun && <RunResult result={lastRun} />}

      {tab === "workflow" && <WorkflowView workflow={workflow} onRunNext={runPrimary} running={Boolean(runningAction)} />}
      {tab === "management" && <ManagementView onRefresh={refresh} />}
      {tab === "artifacts" && <ArtifactsView workflow={workflow} />}
      {tab === "actions" && <ActionsView actions={actions} onRun={runAction} running={runningAction} />}
      {tab === "security" && (
        <div className="twoCols">
          <section className="card"><h3>Security audit</h3><OutputBlock result={snapshot?.security} /></section>
          <section className="card"><h3>Judge verdict</h3><OutputBlock result={workflow?.security_verdict} /></section>
        </div>
      )}
      {tab === "runtime" && (
        <div className="twoCols">
          <section className="card"><h3>Runtime providers</h3><OutputBlock result={snapshot?.runtime} /></section>
          <section className="card"><h3>Git backup</h3><OutputBlock result={snapshot?.git} /></section>
        </div>
      )}
      {tab === "history" && <HistoryView history={history} />}
      {tab === "raw" && (
        <section className="card">
          <h3>Raw snapshot</h3>
          <pre className="output largeOutput">{JSON.stringify(snapshot, null, 2)}</pre>
        </section>
      )}
    </main>
  );
}

export default App;
