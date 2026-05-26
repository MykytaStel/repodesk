import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import "./App.css";

type DebugStatus = "success" | "error";
type LoadStatus = "idle" | "loading" | "ready" | "error";
type ToastKind = "success" | "error" | "info" | "warning";

type TabId =
  | "dashboard"
  | "setup"
  | "workflow"
  | "git"
  | "ai"
  | "actions"
  | "debug"
  | "raw";

interface DebugEvent {
  id: number;
  command: string;
  args?: Record<string, unknown>;
  status: DebugStatus;
  durationMs: number;
  timestamp: string;
  preview?: string;
  error?: string;
}

interface ToastMessage {
  id: number;
  kind: ToastKind;
  title: string;
  message?: string;
}

interface ActionItem {
  id: string;
  label: string;
  description?: string;
  risk?: string;
  category?: string;
}

interface SelfTestItem {
  name: string;
  status: "pass" | "fail";
  detail: string;
}

interface SetupFormState {
  projectName: string;
  projectPath: string;
  projectType: string;
  taskTitle: string;
  taskGoal: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asRecord(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {};
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function getString(value: unknown, key: string, fallback = "—"): string {
  const record = asRecord(value);
  const raw = record[key];
  if (typeof raw === "string" && raw.trim().length > 0) return raw;
  if (typeof raw === "number" || typeof raw === "boolean") return String(raw);
  return fallback;
}

function getNestedString(value: unknown, keys: string[], fallback = "—"): string {
  let current: unknown = value;
  for (const key of keys) {
    if (!isRecord(current)) return fallback;
    current = current[key];
  }
  if (typeof current === "string" && current.trim().length > 0) return current;
  if (typeof current === "number" || typeof current === "boolean") return String(current);
  return fallback;
}

function getNumber(value: unknown, key: string, fallback = 0): number {
  const record = asRecord(value);
  const raw = record[key];
  return typeof raw === "number" && Number.isFinite(raw) ? raw : fallback;
}

function titleize(value: string): string {
  return value.replace(/_/g, " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function stringifyPreview(value: unknown): string {
  if (typeof value === "string") return value.slice(0, 900);
  try {
    return JSON.stringify(value, null, 2).slice(0, 1200);
  } catch {
    return String(value).slice(0, 900);
  }
}

function errorToMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}

function compactJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function normalizeActions(value: unknown): ActionItem[] {
  const rawItems = Array.isArray(value) ? value : asArray(asRecord(value).actions);
  return rawItems.map((item, index) => {
    const record = asRecord(item);
    const id = getString(record, "id", getString(record, "action_id", `action-${index + 1}`));
    return {
      id,
      label: getString(record, "label", titleize(id)),
      description: getString(record, "description", "Bounded RepoDesk action"),
      risk: getString(record, "risk", getString(record, "risk_level", "bounded")),
      category: getString(record, "category", "workflow"),
    };
  });
}

function pickFirstArray(value: unknown, keys: string[]): string[] {
  const record = asRecord(value);
  for (const key of keys) {
    const items = asArray(record[key]);
    if (items.length > 0) return items.map((item) => String(item));
  }
  return [];
}

function getGitGroups(git: unknown) {
  return {
    staged: pickFirstArray(git, ["staged_files", "staged", "cached"]),
    unstaged: pickFirstArray(git, ["unstaged_files", "unstaged", "modified_files", "modified"]),
    untracked: pickFirstArray(git, ["untracked_files", "untracked"]),
  };
}

function gitDirtyCount(git: unknown): number {
  const groups = getGitGroups(git);
  return groups.staged.length + groups.unstaged.length + groups.untracked.length;
}

function gitIsDirty(git: unknown): boolean {
  const record = asRecord(git);
  if (typeof record.is_dirty === "boolean") return record.is_dirty;
  if (typeof record.dirty === "boolean") return record.dirty;
  return gitDirtyCount(git) > 0;
}

function findNextActionId(workflow: unknown, actions: ActionItem[]): string | null {
  const record = asRecord(workflow);
  const explicit = getString(record, "next_action_id", "");
  if (explicit) return explicit;

  const recommended = getString(record, "recommended_action_id", "");
  if (recommended) return recommended;

  const fallbackPriority = [
    "build-smart-context",
    "smart-context-build",
    "build_context",
    "context-build",
    "generate-prompts",
    "prompt-all",
    "safety-scan",
    "checks-run",
    "workflow-next",
  ];

  for (const needle of fallbackPriority) {
    const match = actions.find((action) => action.id.includes(needle));
    if (match) return match.id;
  }

  return actions[0]?.id ?? null;
}

const tabs: Array<{ id: TabId; label: string; subtitle: string }> = [
  { id: "dashboard", label: "Dashboard", subtitle: "Control state" },
  { id: "setup", label: "Setup", subtitle: "Project + task" },
  { id: "workflow", label: "Workflow", subtitle: "Next safe step" },
  { id: "git", label: "Git", subtitle: "Workspace diff" },
  { id: "ai", label: "AI Discovery", subtitle: "Local tools" },
  { id: "actions", label: "Actions", subtitle: "Bounded commands" },
  { id: "debug", label: "Debug", subtitle: "Command traces" },
  { id: "raw", label: "Raw", subtitle: "JSON state" },
];

export default function App() {
  const [activeTab, setActiveTab] = useState<TabId>(() => {
    const saved = window.localStorage.getItem("repodesk.activeTab");
    return tabs.some((tab) => tab.id === saved) ? (saved as TabId) : "dashboard";
  });

  const [loadStatus, setLoadStatus] = useState<LoadStatus>("idle");
  const [busyLabel, setBusyLabel] = useState<string>("");
  const [snapshot, setSnapshot] = useState<unknown>(null);
  const [workflow, setWorkflow] = useState<unknown>(null);
  const [git, setGit] = useState<unknown>(null);
  const [aiDiscovery, setAiDiscovery] = useState<unknown>(null);
  const [actions, setActions] = useState<ActionItem[]>([]);
  const [history, setHistory] = useState<unknown[]>([]);
  const [debugEvents, setDebugEvents] = useState<DebugEvent[]>([]);
  const [toasts, setToasts] = useState<ToastMessage[]>([]);
  const [lastResult, setLastResult] = useState<unknown>(null);
  const [lastActionId, setLastActionId] = useState<string>("");
  const [selfTest, setSelfTest] = useState<SelfTestItem[]>([]);
  const [setupForm, setSetupForm] = useState<SetupFormState>({
    projectName: "repodesk",
    projectPath: "",
    projectType: "rust-tauri",
    taskTitle: "Improve RepoDesk workflow",
    taskGoal: "Make RepoDesk usable as a daily AI development cockpit.",
  });

  const isBooting = loadStatus === "loading" && !snapshot;
  const isBusy = busyLabel.trim().length > 0;
  const projectName = getNestedString(snapshot, ["project", "name"], getString(snapshot, "project_name", "No active project"));
  const taskTitle = getNestedString(snapshot, ["task", "title"], getString(snapshot, "task_title", "No active task"));
  const branch = getString(git, "branch", getString(git, "current_branch", "—"));
  const dirty = gitIsDirty(git);
  const dirtyCount = gitDirtyCount(git);
  const nextActionId = useMemo(() => findNextActionId(workflow, actions), [workflow, actions]);
  const nextAction = actions.find((action) => action.id === nextActionId) ?? null;

  useEffect(() => {
    window.localStorage.setItem("repodesk.activeTab", activeTab);
  }, [activeTab]);

  useEffect(() => {
    void refreshAll("Initial workspace scan");
  }, []);

  function pushToast(kind: ToastKind, title: string, message?: string) {
    const id = Date.now() + Math.random();
    setToasts((items) => [{ id, kind, title, message }, ...items].slice(0, 5));
    window.setTimeout(() => {
      setToasts((items) => items.filter((item) => item.id !== id));
    }, 5200);
  }

  async function callCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    const started = performance.now();
    try {
      const result = await invoke<T>(command, args);
      const durationMs = Math.round(performance.now() - started);
      const event: DebugEvent = {
        id: Date.now() + Math.random(),
        command,
        args,
        status: "success",
        durationMs,
        timestamp: new Date().toLocaleTimeString(),
        preview: stringifyPreview(result),
      };
      setDebugEvents((events) => [event, ...events].slice(0, 120));
      return result;
    } catch (error) {
      const durationMs = Math.round(performance.now() - started);
      const event: DebugEvent = {
        id: Date.now() + Math.random(),
        command,
        args,
        status: "error",
        durationMs,
        timestamp: new Date().toLocaleTimeString(),
        error: errorToMessage(error),
      };
      setDebugEvents((events) => [event, ...events].slice(0, 120));
      throw error;
    }
  }

  async function optionalCommand<T>(command: string, args?: Record<string, unknown>): Promise<T | null> {
    try {
      return await callCommand<T>(command, args);
    } catch {
      return null;
    }
  }

  async function refreshAll(label = "Refreshing") {
    setLoadStatus((status) => (status === "idle" ? "loading" : status));
    setBusyLabel(label);
    try {
      const [snapshotResult, workflowResult, gitResult, aiResult, actionsResult, historyResult] = await Promise.all([
        optionalCommand<unknown>("desktop_snapshot"),
        optionalCommand<unknown>("product_workflow_state"),
        optionalCommand<unknown>("git_workspace_snapshot"),
        optionalCommand<unknown>("ai_discovery_scan"),
        optionalCommand<unknown>("desktop_actions"),
        optionalCommand<unknown[]>("action_history"),
      ]);

      setSnapshot(snapshotResult);
      setWorkflow(workflowResult);
      setGit(gitResult);
      setAiDiscovery(aiResult);
      setActions(normalizeActions(actionsResult));
      setHistory(Array.isArray(historyResult) ? historyResult : []);
      setLoadStatus("ready");
    } catch (error) {
      setLoadStatus("error");
      pushToast("error", "Refresh failed", errorToMessage(error));
    } finally {
      setBusyLabel("");
    }
  }

  async function runAction(actionId: string) {
    setBusyLabel(`Running ${actionId}`);
    setLastActionId(actionId);
    try {
      const result = await callCommand<unknown>("run_desktop_action", { actionId, action_id: actionId });
      setLastResult(result);
      pushToast("success", "Action completed", actionId);
      await refreshAll("Refreshing after action");
    } catch (error) {
      setLastResult({ actionId, error: errorToMessage(error) });
      pushToast("error", "Action failed", errorToMessage(error));
    } finally {
      setBusyLabel("");
    }
  }

  async function doNextSafeStep() {
    if (!nextActionId) {
      pushToast("warning", "No next action", "Create a task or refresh the workflow state first.");
      setActiveTab("setup");
      return;
    }

    if (dirty && nextActionId.toLowerCase().includes("agent")) {
      pushToast("warning", "Dirty workspace", "Review Git changes before running an agent action.");
      setActiveTab("git");
      return;
    }

    await runAction(nextActionId);
  }

  async function runSelfTest() {
    setBusyLabel("Running product self-test");
    const checks: Array<{ name: string; command: string }> = [
      { name: "Desktop snapshot", command: "desktop_snapshot" },
      { name: "Workflow state", command: "product_workflow_state" },
      { name: "Git workspace", command: "git_workspace_snapshot" },
      { name: "AI discovery", command: "ai_discovery_scan" },
      { name: "Desktop actions", command: "desktop_actions" },
    ];

    const results: SelfTestItem[] = [];
    for (const check of checks) {
      try {
        const result = await callCommand<unknown>(check.command);
        results.push({ name: check.name, status: "pass", detail: stringifyPreview(result).slice(0, 160) });
      } catch (error) {
        results.push({ name: check.name, status: "fail", detail: errorToMessage(error) });
      }
    }

    setSelfTest(results);
    setBusyLabel("");
    const failed = results.filter((item) => item.status === "fail").length;
    pushToast(failed === 0 ? "success" : "warning", "Self-test finished", `${results.length - failed}/${results.length} checks passed`);
  }

  async function callFirst(candidates: Array<{ command: string; args?: Record<string, unknown> }>) {
    const errors: string[] = [];
    for (const candidate of candidates) {
      try {
        return await callCommand<unknown>(candidate.command, candidate.args);
      } catch (error) {
        errors.push(`${candidate.command}: ${errorToMessage(error)}`);
      }
    }
    throw new Error(errors.join("\n"));
  }

  async function addProjectFromSetup() {
    if (!setupForm.projectName.trim() || !setupForm.projectPath.trim()) {
      pushToast("warning", "Project data missing", "Add project name and absolute path.");
      return;
    }

    setBusyLabel("Adding project");
    try {
      await callFirst([
        {
          command: "add_project_from_ui",
          args: {
            name: setupForm.projectName.trim(),
            path: setupForm.projectPath.trim(),
            project_type: setupForm.projectType.trim(),
            main_language: "rust",
          },
        },
        {
          command: "desktop_add_project",
          args: {
            name: setupForm.projectName.trim(),
            path: setupForm.projectPath.trim(),
            project_type: setupForm.projectType.trim(),
          },
        },
      ]);
      pushToast("success", "Project connected", setupForm.projectName.trim());
      await refreshAll("Refreshing project state");
    } catch (error) {
      pushToast("error", "Could not add project", errorToMessage(error));
    } finally {
      setBusyLabel("");
    }
  }

  async function createTaskFromSetup() {
    if (!setupForm.taskTitle.trim()) {
      pushToast("warning", "Task title missing", "Add a clear task title first.");
      return;
    }

    setBusyLabel("Creating task");
    try {
      await callFirst([
        {
          command: "create_task_from_ui",
          args: {
            title: setupForm.taskTitle.trim(),
            goal: setupForm.taskGoal.trim(),
          },
        },
        {
          command: "desktop_create_task",
          args: {
            title: setupForm.taskTitle.trim(),
            goal: setupForm.taskGoal.trim(),
          },
        },
      ]);
      pushToast("success", "Task created", setupForm.taskTitle.trim());
      await refreshAll("Refreshing task state");
      setActiveTab("workflow");
    } catch (error) {
      pushToast("error", "Could not create task", errorToMessage(error));
    } finally {
      setBusyLabel("");
    }
  }

  function renderDashboard() {
    const readiness = [
      { label: "Project connected", ok: projectName !== "No active project" },
      { label: "Task selected", ok: taskTitle !== "No active task" },
      { label: "Git visible", ok: Boolean(git) },
      { label: "AI discovery visible", ok: Boolean(aiDiscovery) },
      { label: "Workflow actions loaded", ok: actions.length > 0 },
    ];

    return (
      <div className="page-grid">
        <section className="hero-card">
          <div>
            <p className="eyebrow">RepoDesk Control Brain</p>
            <h1>One safe workflow for local AI development.</h1>
            <p className="hero-copy">
              Connect a project, create a task, build bounded context, inspect Git changes, then route work to the right AI/runtime with guardrails.
            </p>
          </div>
          <div className="hero-actions">
            <button className="primary-button" onClick={() => void doNextSafeStep()} disabled={isBusy}>
              {nextAction ? `Do next: ${nextAction.label}` : "Do next safe step"}
            </button>
            <button className="secondary-button" onClick={() => void refreshAll("Manual refresh")} disabled={isBusy}>
              Refresh workspace
            </button>
          </div>
        </section>

        <section className="card span-2">
          <div className="section-heading">
            <div>
              <p className="eyebrow">Readiness</p>
              <h2>Daily cockpit checklist</h2>
            </div>
            <span className={`pill ${readiness.every((item) => item.ok) ? "ok" : "warn"}`}>{readiness.filter((item) => item.ok).length}/{readiness.length} ready</span>
          </div>
          <div className="checklist-grid">
            {readiness.map((item) => (
              <div key={item.label} className={`check-row ${item.ok ? "ok" : "warn"}`}>
                <span>{item.ok ? "✓" : "!"}</span>
                <p>{item.label}</p>
              </div>
            ))}
          </div>
        </section>

        <section className="card">
          <p className="eyebrow">Current project</p>
          <h2>{projectName}</h2>
          <p className="muted">Task: {taskTitle}</p>
          <p className="muted">Branch: {branch}</p>
        </section>

        <section className="card">
          <p className="eyebrow">Workspace safety</p>
          <h2>{dirty ? `${dirtyCount} pending changes` : "Clean workspace"}</h2>
          <p className="muted">{dirty ? "Review Git before running file-changing agent actions." : "No visible pending Git changes."}</p>
        </section>

        <section className="card">
          <p className="eyebrow">Last action</p>
          <h2>{lastActionId || "No action yet"}</h2>
          <pre className="mini-pre">{lastResult ? stringifyPreview(lastResult) : "Run a bounded action to see its result here."}</pre>
        </section>
      </div>
    );
  }

  function renderSetup() {
    return (
      <div className="page-grid two-col">
        <section className="card">
          <p className="eyebrow">Setup</p>
          <h2>Connect project</h2>
          <p className="muted">Use an absolute local path. RepoDesk stores metadata only; it does not push or commit anything.</p>
          <label className="field-label">Project name</label>
          <input value={setupForm.projectName} onChange={(event) => setSetupForm({ ...setupForm, projectName: event.target.value })} placeholder="repodesk" />
          <label className="field-label">Project path</label>
          <input value={setupForm.projectPath} onChange={(event) => setSetupForm({ ...setupForm, projectPath: event.target.value })} placeholder="/Users/mykyta/Documents/projects/repodesk" />
          <label className="field-label">Project type</label>
          <input value={setupForm.projectType} onChange={(event) => setSetupForm({ ...setupForm, projectType: event.target.value })} placeholder="rust-tauri" />
          <button className="primary-button full" onClick={() => void addProjectFromSetup()} disabled={isBusy}>Add and activate project</button>
        </section>

        <section className="card">
          <p className="eyebrow">Task</p>
          <h2>Create active task</h2>
          <p className="muted">A task is the work unit used for context, prompts, checks, receipts and history.</p>
          <label className="field-label">Task title</label>
          <input value={setupForm.taskTitle} onChange={(event) => setSetupForm({ ...setupForm, taskTitle: event.target.value })} />
          <label className="field-label">Goal</label>
          <textarea value={setupForm.taskGoal} onChange={(event) => setSetupForm({ ...setupForm, taskGoal: event.target.value })} rows={6} />
          <button className="primary-button full" onClick={() => void createTaskFromSetup()} disabled={isBusy}>Create task</button>
        </section>
      </div>
    );
  }

  function renderWorkflow() {
    const steps = [
      { key: "project", label: "Project", done: projectName !== "No active project" },
      { key: "task", label: "Task", done: taskTitle !== "No active task" },
      { key: "git", label: "Git visible", done: Boolean(git) },
      { key: "context", label: "Context", done: getString(workflow, "context_status", "") === "ready" || Boolean(getNestedString(snapshot, ["artifacts", "context"], "")) },
      { key: "safety", label: "Safety", done: Boolean(getString(workflow, "safety_status", "")) || Boolean(snapshot) },
      { key: "prompts", label: "Prompts", done: getNumber(workflow, "prompts_count", 0) > 0 || getNumber(snapshot, "prompts_count", 0) > 0 },
      { key: "checks", label: "Checks", done: Boolean(getString(workflow, "checks_status", "")) || Boolean(getString(snapshot, "checks_status", "")) },
    ];

    return (
      <div className="page-grid">
        <section className="hero-card span-2">
          <div>
            <p className="eyebrow">Workflow</p>
            <h1>Run the next safe step, not random commands.</h1>
            <p className="hero-copy">RepoDesk should guide your AI workflow: context first, safety second, prompt/actions only when the workspace is understood.</p>
          </div>
          <button className="primary-button" onClick={() => void doNextSafeStep()} disabled={isBusy}>{nextAction ? nextAction.label : "Do next safe step"}</button>
        </section>

        <section className="card span-2">
          <div className="timeline">
            {steps.map((step, index) => (
              <div key={step.key} className={`timeline-step ${step.done ? "done" : "todo"}`}>
                <span>{step.done ? "✓" : index + 1}</span>
                <p>{step.label}</p>
              </div>
            ))}
          </div>
        </section>

        <section className="card">
          <p className="eyebrow">Next recommendation</p>
          <h2>{nextAction?.label ?? "No action loaded"}</h2>
          <p className="muted">{nextAction?.description ?? "Refresh or create a task to load workflow actions."}</p>
          {dirty && <p className="warning-box">Workspace has pending Git changes. Review before running agent actions.</p>}
        </section>

        <section className="card">
          <p className="eyebrow">Self-test</p>
          <h2>Product health check</h2>
          <button className="secondary-button full" onClick={() => void runSelfTest()} disabled={isBusy}>Run product self-test</button>
          <div className="self-test-list">
            {selfTest.map((item) => (
              <div key={item.name} className={`self-test-row ${item.status}`}>
                <strong>{item.status === "pass" ? "✓" : "!"} {item.name}</strong>
                <span>{item.detail}</span>
              </div>
            ))}
          </div>
        </section>
      </div>
    );
  }

  function renderGit() {
    const groups = getGitGroups(git);
    const diffStat = getString(git, "diff_stat", getString(git, "stat", "No diff stat available"));

    return (
      <div className="page-grid">
        <section className="hero-card span-2">
          <div>
            <p className="eyebrow">Git Workspace</p>
            <h1>{dirty ? `${dirtyCount} pending changes` : "Workspace is clean"}</h1>
            <p className="hero-copy">RepoDesk only reads Git state here. It does not stage, commit, reset or push.</p>
          </div>
          <button className="secondary-button" onClick={() => void refreshAll("Refreshing Git state")} disabled={isBusy}>Refresh Git</button>
        </section>

        <section className="card">
          <p className="eyebrow">Branch</p>
          <h2>{branch}</h2>
          <p className="muted">Last commit: {getString(git, "last_commit", "—")}</p>
        </section>

        <section className="card span-2">
          <p className="eyebrow">Diff stat</p>
          <pre className="code-panel">{diffStat}</pre>
        </section>

        {([
          ["Staged", groups.staged],
          ["Unstaged", groups.unstaged],
          ["Untracked", groups.untracked],
        ] as Array<[string, string[]]>).map(([label, files]) => (
          <section className="card" key={label}>
            <div className="section-heading compact">
              <h2>{label}</h2>
              <span className="pill">{files.length}</span>
            </div>
            <div className="file-list">
              {files.length === 0 ? <p className="muted">No files.</p> : files.map((file) => <code key={file}>{file}</code>)}
            </div>
          </section>
        ))}
      </div>
    );
  }

  function renderAiDiscovery() {
    const record = asRecord(aiDiscovery);
    const tools = asArray(record.tools ?? record.items ?? record.discovered);
    const endpoints = asArray(record.endpoints ?? record.local_endpoints);
    const found = tools.filter((tool) => {
      const toolRecord = asRecord(tool);
      return Boolean(toolRecord.available ?? toolRecord.found ?? toolRecord.installed);
    });
    const missing = tools.filter((tool) => !found.includes(tool));

    return (
      <div className="page-grid">
        <section className="hero-card span-2">
          <div>
            <p className="eyebrow">AI Discovery</p>
            <h1>{found.length} local AI/runtime tools found</h1>
            <p className="hero-copy">Passive scan only: PATH lookup, known app paths, localhost health checks. No secrets, no outbound upload.</p>
          </div>
          <button className="secondary-button" onClick={() => void refreshAll("Scanning AI runtime")}>Scan again</button>
        </section>

        <section className="card">
          <p className="eyebrow">Found</p>
          <div className="tool-list">
            {found.length === 0 ? <p className="muted">Nothing found yet.</p> : found.map((tool, index) => <ToolRow key={index} value={tool} />)}
          </div>
        </section>

        <section className="card">
          <p className="eyebrow">Missing / not detected</p>
          <div className="tool-list">
            {missing.length === 0 ? <p className="muted">No missing tools reported.</p> : missing.map((tool, index) => <ToolRow key={index} value={tool} />)}
          </div>
        </section>

        <section className="card span-2">
          <p className="eyebrow">Local endpoints</p>
          <div className="tool-list horizontal">
            {endpoints.length === 0 ? <p className="muted">No endpoint data.</p> : endpoints.map((endpoint, index) => <ToolRow key={index} value={endpoint} />)}
          </div>
        </section>
      </div>
    );
  }

  function renderActions() {
    return (
      <div className="page-grid">
        <section className="hero-card span-2">
          <div>
            <p className="eyebrow">Bounded Actions</p>
            <h1>UI can only call allowed RepoDesk actions.</h1>
            <p className="hero-copy">No unrestricted shell is exposed to the desktop app. Every action should be explicit, logged and visible in Debug.</p>
          </div>
        </section>

        <section className="card span-2">
          <div className="action-grid">
            {actions.length === 0 ? <p className="muted">No actions loaded. Refresh workspace.</p> : actions.map((action) => (
              <article key={action.id} className="action-card">
                <div>
                  <p className="eyebrow">{action.category}</p>
                  <h3>{action.label}</h3>
                  <p>{action.description}</p>
                  <span className={`pill ${action.risk === "high" ? "danger" : action.risk === "medium" ? "warn" : "ok"}`}>{action.risk}</span>
                </div>
                <button className="secondary-button" onClick={() => void runAction(action.id)} disabled={isBusy}>Run</button>
              </article>
            ))}
          </div>
        </section>

        <section className="card span-2">
          <p className="eyebrow">Action history</p>
          <pre className="code-panel tall">{history.length === 0 ? "No action history yet." : compactJson(history)}</pre>
        </section>
      </div>
    );
  }

  function renderDebug() {
    return (
      <div className="page-grid">
        <section className="hero-card span-2">
          <div>
            <p className="eyebrow">Debug Console</p>
            <h1>Every Tauri call should be visible.</h1>
            <p className="hero-copy">Use this when you do not know if something worked. Success, errors and duration are tracked here.</p>
          </div>
          <button className="secondary-button" onClick={() => setDebugEvents([])}>Clear</button>
        </section>

        <section className="card span-2">
          <div className="debug-list">
            {debugEvents.length === 0 ? <p className="muted">No commands called yet.</p> : debugEvents.map((event) => (
              <article key={event.id} className={`debug-row ${event.status}`}>
                <div className="debug-main">
                  <strong>{event.command}</strong>
                  <span>{event.timestamp} · {event.durationMs}ms</span>
                </div>
                <pre>{event.error ?? event.preview ?? "No preview"}</pre>
              </article>
            ))}
          </div>
        </section>
      </div>
    );
  }

  function renderRaw() {
    return (
      <div className="page-grid">
        <section className="card span-2">
          <p className="eyebrow">Raw snapshot</p>
          <pre className="code-panel tall">{compactJson({ snapshot, workflow, git, aiDiscovery, actions })}</pre>
        </section>
      </div>
    );
  }

  function renderActiveTab() {
    if (isBooting) return <StartupSkeleton />;
    if (activeTab === "dashboard") return renderDashboard();
    if (activeTab === "setup") return renderSetup();
    if (activeTab === "workflow") return renderWorkflow();
    if (activeTab === "git") return renderGit();
    if (activeTab === "ai") return renderAiDiscovery();
    if (activeTab === "actions") return renderActions();
    if (activeTab === "debug") return renderDebug();
    return renderRaw();
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand-block">
          <div className="brand-mark">RD</div>
          <div>
            <strong>RepoDesk</strong>
            <span>AI control cockpit</span>
          </div>
        </div>
        <nav className="nav-list">
          {tabs.map((tab) => (
            <button key={tab.id} className={activeTab === tab.id ? "active" : ""} onClick={() => setActiveTab(tab.id)}>
              <strong>{tab.label}</strong>
              <span>{tab.subtitle}</span>
            </button>
          ))}
        </nav>
      </aside>

      <main className="main-shell">
        <header className="topbar">
          <div>
            <p className="eyebrow">Active workspace</p>
            <h1>{projectName}</h1>
          </div>
          <div className="status-strip">
            <StatusChip label="Task" value={taskTitle} state={taskTitle === "No active task" ? "warn" : "ok"} />
            <StatusChip label="Git" value={dirty ? `${dirtyCount} changes` : "clean"} state={dirty ? "warn" : "ok"} />
            <StatusChip label="Next" value={nextAction?.label ?? "setup"} state={nextAction ? "ok" : "warn"} />
          </div>
        </header>

        {renderActiveTab()}
      </main>

      {isBusy && <div className="loading-overlay"><div className="spinner" /><strong>{busyLabel}</strong><span>RepoDesk is working. Watch Debug for command traces.</span></div>}

      <div className="toast-stack">
        {toasts.map((toast) => (
          <div key={toast.id} className={`toast ${toast.kind}`}>
            <strong>{toast.title}</strong>
            {toast.message && <span>{toast.message}</span>}
          </div>
        ))}
      </div>
    </div>
  );
}

function ToolRow({ value }: { value: unknown }) {
  const record = asRecord(value);
  const name = getString(record, "name", getString(record, "id", stringifyPreview(value).slice(0, 60)));
  const detail = getString(record, "path", getString(record, "url", getString(record, "detail", "")));
  const available = Boolean(record.available ?? record.found ?? record.installed ?? record.open);
  return (
    <div className="tool-row">
      <span className={`dot ${available ? "ok" : "warn"}`} />
      <div>
        <strong>{name}</strong>
        {detail && <small>{detail}</small>}
      </div>
    </div>
  );
}

function StatusChip({ label, value, state }: { label: string; value: string; state: "ok" | "warn" | "danger" }) {
  return (
    <div className={`status-chip ${state}`}>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function StartupSkeleton() {
  return (
    <div className="page-grid">
      <section className="hero-card span-2 skeleton-block">
        <div className="skeleton-line wide" />
        <div className="skeleton-line" />
        <div className="skeleton-line short" />
      </section>
      {Array.from({ length: 6 }).map((_, index) => (
        <section className="card skeleton-block" key={index}>
          <div className="skeleton-line" />
          <div className="skeleton-line short" />
          <div className="skeleton-line wide" />
        </section>
      ))}
    </div>
  );
}
