import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import "./App.css";

type TabId = "dashboard" | "workflow" | "tokens" | "models" | "code" | "git" | "settings" | "system" | "debug";
type DebugStatus = "success" | "error";
type ToastKind = "success" | "error" | "warning" | "info";
type UnknownRecord = Record<string, unknown>;

interface DebugEvent {
  id: number;
  command: string;
  args?: UnknownRecord;
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
  title: string;
  description: string;
  risk: string;
  category: string;
}

interface ProviderSettings {
  ollama_enabled: boolean;
  ollama_url: string;
  ollama_model: string;
  lm_studio_enabled: boolean;
  lm_studio_url: string;
  llamafile_enabled: boolean;
  llamafile_url: string;
  localai_enabled: boolean;
  localai_url: string;
  chatgpt_enabled: boolean;
  codex_enabled: boolean;
  gemini_enabled: boolean;
  openai_api_enabled: boolean;
  openai_api_key_env_var: string;
  gemini_api_enabled: boolean;
  gemini_api_key_env_var: string;
  allow_paid_agents: boolean;
  codex_quota_status: string;
  preferred_patch_provider: string;
  preferred_compression_provider: string;
  preferred_review_provider: string;
  notes: string;
}

interface RouteCandidate {
  provider: string;
  label: string;
  kind: string;
  model?: string | null;
  score: number;
  blocked: boolean;
  blockers: string[];
  warnings: string[];
  required_guardrails: string[];
  estimated_cost_units: number;
}

interface RouteDecision {
  task_kind: string;
  recommended_provider: string;
  recommended_model?: string | null;
  fallback_provider?: string | null;
  fallback_model?: string | null;
  score: number;
  decision_level: string;
  blockers: string[];
  warnings: string[];
  required_guardrails: string[];
  candidates: RouteCandidate[];
  estimated_total_tokens: number;
}

interface RoutingSnapshot {
  generated_at_ms: number;
  request: {
    task_kind: string;
    estimated_input_tokens: number;
    estimated_output_tokens: number;
    risk_level: string;
    changed_file_count: number;
    requires_write: boolean;
    context_safe?: boolean | null;
    checks_ok?: boolean | null;
    guard_allowed?: boolean | null;
    git_dirty?: boolean | null;
    max_cost_units?: number | null;
  };
  decision: RouteDecision;
  capacities: Array<{
    provider: string;
    label: string;
    kind: string;
    enabled: boolean;
    auth_status: string;
    reachability: string;
    models: string[];
    preferred_model?: string | null;
    daily_remaining_tokens: number;
    estimated_cost_units: number;
    quota_status: string;
    paid_agents_allowed: boolean;
    max_patch_files: number;
  }>;
}

interface TokenUsageSnapshot {
  generated_at_ms: number;
  totals: {
    entries_count: number;
    total_input_tokens: number;
    total_output_tokens: number;
    total_tokens: number;
    today_total_tokens: number;
    remaining_daily_tokens: number;
  };
  by_provider: TokenUsageItem[];
  by_model: TokenUsageItem[];
  active_artifacts: TokenArtifactEstimate[];
  cost_estimate: {
    estimated_total_units: number;
    currency_label: string;
    note: string;
  };
}

interface ApiEnvDiagnostic {
  openai_api_key_set: boolean;
  gemini_api_key_set: boolean;
  anthropic_api_key_set: boolean;
}

interface TokenUsageItem {
  provider: string;
  model?: string | null;
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
  estimated_cost_units?: number | null;
  currency_label?: string | null;
}

interface TokenArtifactEstimate {
  kind: string;
  title: string;
  path?: string | null;
  exists: boolean;
  size_bytes: number;
  estimated_tokens?: number | null;
  status: string;
  recommendation: string;
  error?: string | null;
}

interface ModelHealthSnapshot {
  generated_at_ms: number;
  providers: ProviderHealth[];
  warnings: string[];
}

interface ProviderHealth {
  id: string;
  label: string;
  enabled: boolean;
  auth_status: string;
  reachability: "working" | "auth_missing" | "unreachable" | "rate_limited" | "disabled" | string;
  models: ModelStatus[];
  error_summary?: string | null;
}

interface ModelStatus {
  id: string;
  provider: string;
  available: boolean;
  loaded?: boolean | null;
  context_window?: number | null;
  notes?: string | null;
}

interface AgentConfig {
  name: string;
  kind: string;
  role: string;
  default_budget_tokens: number;
  allowed_actions: string[];
  forbidden_actions: string[];
  preferred_for: string[];
}

interface AgentsConfig {
  agents: AgentConfig[];
}

interface Capability {
  name: string;
  kind: string;
  enabled: boolean;
  local: boolean;
  risk: string;
  boundary: string;
  preferred_for: string[];
  allowed_actions: string[];
  forbidden_actions: string[];
}

interface CapabilitiesConfig {
  capabilities: Capability[];
}

interface PeripheralConfig {
  name: string;
  kind: string;
  access: string;
  risk: string;
  allowed_actions: string[];
  forbidden_actions: string[];
}

interface PeripheralsConfig {
  peripherals: PeripheralConfig[];
}

interface BrainModule {
  name: string;
  layer: string;
  status: string;
  purpose: string;
}

interface SetupFormState {
  projectName: string;
  projectPath: string;
  projectType: string;
  mainLanguage: string;
  taskTitle: string;
}

interface TokenLogFormState {
  provider: string;
  model: string;
  inputTokens: string;
  outputTokens: string;
  category: string;
  notes: string;
}

const tabs: Array<{ id: TabId; title: string; subtitle: string }> = [
  { id: "dashboard", title: "Dashboard", subtitle: "Daily state" },
  { id: "workflow", title: "Workflow", subtitle: "Next step" },
  { id: "tokens", title: "Tokens", subtitle: "Usage + cost" },
  { id: "models", title: "Models", subtitle: "Runtime health" },
  { id: "code", title: "Code", subtitle: "Changed files" },
  { id: "git", title: "Git", subtitle: "Workspace" },
  { id: "settings", title: "Settings", subtitle: "Providers" },
  { id: "system", title: "System Registry", subtitle: "Skills & MCP" },
  { id: "debug", title: "Debug", subtitle: "Traces" },
];

const defaultProviderSettings: ProviderSettings = {
  ollama_enabled: true,
  ollama_url: "http://127.0.0.1:11434",
  ollama_model: "llama3.1",
  lm_studio_enabled: true,
  lm_studio_url: "http://127.0.0.1:1234",
  llamafile_enabled: false,
  llamafile_url: "http://127.0.0.1:8080",
  localai_enabled: false,
  localai_url: "http://127.0.0.1:8080",
  chatgpt_enabled: true,
  codex_enabled: true,
  gemini_enabled: false,
  openai_api_enabled: true,
  openai_api_key_env_var: "OPENAI_API_KEY",
  gemini_api_enabled: false,
  gemini_api_key_env_var: "GEMINI_API_KEY",
  allow_paid_agents: true,
  codex_quota_status: "unknown",
  preferred_patch_provider: "codex",
  preferred_compression_provider: "ollama",
  preferred_review_provider: "chatgpt",
  notes: "Local-first by default. Paid agents should receive bounded smart context only.",
};

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asRecord(value: unknown): UnknownRecord {
  return isRecord(value) ? value : {};
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function getValue(source: unknown, key: string): unknown {
  return asRecord(source)[key];
}

function getString(source: unknown, key: string, fallback = "-"): string {
  const value = getValue(source, key);
  if (typeof value === "string" && value.trim()) return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return fallback;
}

function getNestedString(source: unknown, path: string[], fallback = "-"): string {
  let value: unknown = source;
  for (const segment of path) value = asRecord(value)[segment];
  if (typeof value === "string" && value.trim()) return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return fallback;
}

function stringifyPreview(value: unknown, max = 4000): string {
  let text: string;
  if (typeof value === "string") text = value;
  else {
    try {
      text = JSON.stringify(value, null, 2);
    } catch {
      text = String(value);
    }
  }
  return text.length > max ? `${text.slice(0, max)}\n\n[truncated]` : text;
}

function errorToMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return stringifyPreview(error, 1200);
}

function formatNumber(value: number | undefined | null): string {
  return typeof value === "number" && Number.isFinite(value) ? value.toLocaleString() : "-";
}

function formatCost(value: number | undefined | null, currency?: string | null): string {
  if (typeof value !== "number" || !Number.isFinite(value)) return "-";
  return `${value.toFixed(4)} ${currency || "cost_units"}`;
}

function listFromRecord(source: unknown, keys: string[]): string[] {
  const record = asRecord(source);
  for (const key of keys) {
    const value = record[key];
    if (Array.isArray(value)) {
      return value.map((item) => {
        if (typeof item === "string") return item;
        const itemRecord = asRecord(item);
        return getString(itemRecord, "path", getString(itemRecord, "name", stringifyPreview(item, 160)));
      });
    }
  }
  return [];
}

function gitDirtyCount(git: unknown): number {
  return (
    listFromRecord(git, ["staged", "staged_files"]).length +
    listFromRecord(git, ["unstaged", "unstaged_files", "modified_files"]).length +
    listFromRecord(git, ["untracked", "untracked_files"]).length
  );
}

function gitIsDirty(git: unknown): boolean {
  const explicit = getValue(git, "is_dirty");
  if (typeof explicit === "boolean") return explicit;
  const clean = getValue(git, "clean");
  if (typeof clean === "boolean") return !clean;
  return gitDirtyCount(git) > 0;
}

function codeChangedFiles(code: unknown): string[] {
  const direct = listFromRecord(code, ["changed_files", "files"]);
  if (direct.length > 0) return direct;
  return [...listFromRecord(code, ["staged"]), ...listFromRecord(code, ["unstaged"]), ...listFromRecord(code, ["untracked"])];
}

function normalizeActions(value: unknown): ActionItem[] {
  return asArray(value).map((item, index) => {
    const record = asRecord(item);
    const title = getString(record, "title", getString(record, "label", `Action ${index + 1}`));
    return {
      id: getString(record, "id", `action-${index}`),
      label: title,
      title,
      description: getString(record, "description", "No description."),
      risk: getString(record, "risk", "safe"),
      category: getString(record, "category", "General"),
    };
  });
}

function statusTone(value: string | boolean | undefined | null): "ok" | "warn" | "danger" | "neutral" {
  if (typeof value === "boolean") return value ? "ok" : "warn";
  const lower = String(value || "").toLowerCase();
  if (["working", "ok", "done", "safe", "configured", "not_required"].some((item) => lower.includes(item))) return "ok";
  if (["disabled", "missing", "unreachable", "rate", "warn", "large"].some((item) => lower.includes(item))) return "warn";
  if (["block", "danger", "error", "failed", "too large"].some((item) => lower.includes(item))) return "danger";
  return "neutral";
}

function findNextActionId(workflow: unknown, actions: ActionItem[], hasProject: boolean, hasTask: boolean): string {
  if (!hasProject || !hasTask) return "";
  const explicit = getString(workflow, "recommended_action_id", getString(workflow, "next_action_id", ""));
  if (explicit && actions.some((action) => action.id === explicit)) return explicit;
  const preferred = ["smart-context-build", "context-build", "safety-scan-context", "prompt-all", "checks-run", "workflow-next"];
  return preferred.find((id) => actions.some((action) => action.id === id)) ?? actions[0]?.id ?? "";
}

async function copyToClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const textarea = document.createElement("textarea");
    textarea.value = text;
    document.body.appendChild(textarea);
    textarea.select();
    document.execCommand("copy");
    textarea.remove();
  }
}

export default function App() {
  const [activeTab, setActiveTab] = useState<TabId>(() => (window.localStorage.getItem("repodesk.activeTab") as TabId) || "dashboard");
  const [booting, setBooting] = useState(true);
  const [busyLabel, setBusyLabel] = useState("");
  const [snapshot, setSnapshot] = useState<unknown>(null);
  const [workflow, setWorkflow] = useState<unknown>(null);
  const [git, setGit] = useState<unknown>(null);
  const [codeWorkbench, setCodeWorkbench] = useState<unknown>(null);
  const [actions, setActions] = useState<ActionItem[]>([]);
  const [history, setHistory] = useState<unknown[]>([]);
  const [tokens, setTokens] = useState<TokenUsageSnapshot | null>(null);
  const [models, setModels] = useState<ModelHealthSnapshot | null>(null);
  const [routing, setRouting] = useState<RoutingSnapshot | null>(null);
  const [apiEnvDiagnostic, setApiEnvDiagnostic] = useState<ApiEnvDiagnostic | null>(null);
  const [systemAgents, setSystemAgents] = useState<AgentsConfig | null>(null);
  const [systemCapabilities, setSystemCapabilities] = useState<CapabilitiesConfig | null>(null);
  const [systemPeripherals, setSystemPeripherals] = useState<PeripheralsConfig | null>(null);
  const [systemModules, setSystemModules] = useState<BrainModule[]>([]);
  const [providerSettings, setProviderSettings] = useState<ProviderSettings>(defaultProviderSettings);
  const [dbState, setDbState] = useState<unknown>(null);
  const [debugEvents, setDebugEvents] = useState<DebugEvent[]>([]);
  const [toasts, setToasts] = useState<ToastMessage[]>([]);
  const [lastResult, setLastResult] = useState<unknown>(null);
  const [selectedFile, setSelectedFile] = useState("");
  const [selectedFileContent, setSelectedFileContent] = useState("");
  const [artifactKind, setArtifactKind] = useState("smart_context");
  const [artifactContent, setArtifactContent] = useState("");
  const [projectConfig, setProjectConfig] = useState<any>(null);
  const [fileTokenEstimates, setFileTokenEstimates] = useState<any[]>([]);
  const [projectMemory, setProjectMemory] = useState("");
  const [memoryAppendInput, setMemoryAppendInput] = useState("");
  const [setupForm, setSetupForm] = useState<SetupFormState>({
    projectName: "repodesk",
    projectPath: "",
    projectType: "rust-tauri",
    mainLanguage: "rust",
    taskTitle: "Improve RepoDesk workflow",
  });
  const [tokenLogForm, setTokenLogForm] = useState<TokenLogFormState>({
    provider: "manual",
    model: "",
    inputTokens: "0",
    outputTokens: "0",
    category: "general",
    notes: "",
  });

  const projectName = getNestedString(snapshot, ["project", "name"], getString(snapshot, "project_name", "No active project"));
  const taskTitle = getNestedString(snapshot, ["task", "title"], getString(snapshot, "task_title", "No active task"));
  const hasProject = projectName !== "No active project" && projectName !== "-";
  const hasTask = taskTitle !== "No active task" && taskTitle !== "-";
  const branch = getString(git, "branch", getString(git, "current_branch", "-"));
  const dirty = gitIsDirty(git);
  const dirtyCount = gitDirtyCount(git);
  const changedFiles = codeChangedFiles(codeWorkbench);
  const nextActionId = useMemo(() => findNextActionId(workflow, actions, hasProject, hasTask), [workflow, actions, hasProject, hasTask]);
  const nextAction = actions.find((action) => action.id === nextActionId) ?? null;
  const workingProviders = models?.providers.filter((provider) => provider.reachability === "working").length ?? 0;
  const modelCount = models?.providers.reduce((total, provider) => total + provider.models.length, 0) ?? 0;
  const isBusy = busyLabel.trim().length > 0;

  useEffect(() => {
    window.localStorage.setItem("repodesk.activeTab", activeTab);
  }, [activeTab]);

  useEffect(() => {
    void refreshAll("Starting RepoDesk");
  }, []);

  function pushToast(kind: ToastKind, title: string, message?: string) {
    const id = Date.now() + Math.random();
    setToasts((items) => [{ id, kind, title, message }, ...items].slice(0, 5));
    window.setTimeout(() => setToasts((items) => items.filter((item) => item.id !== id)), 4500);
  }

  async function callCommand<T>(command: string, args?: UnknownRecord): Promise<T> {
    const started = performance.now();
    try {
      const result = await invoke<T>(command, args);
      const durationMs = Math.round(performance.now() - started);
      setDebugEvents((events) => [{
        id: Date.now() + Math.random(),
        command,
        args,
        status: "success" as const,
        durationMs,
        timestamp: new Date().toLocaleTimeString(),
        preview: stringifyPreview(result, 1200),
      }, ...events].slice(0, 150));
      return result;
    } catch (error) {
      const durationMs = Math.round(performance.now() - started);
      setDebugEvents((events) => [{
        id: Date.now() + Math.random(),
        command,
        args,
        status: "error" as const,
        durationMs,
        timestamp: new Date().toLocaleTimeString(),
        error: errorToMessage(error),
      }, ...events].slice(0, 150));
      throw error;
    }
  }

  async function optionalCommand<T>(command: string, args?: UnknownRecord): Promise<T | null> {
    try {
      return await callCommand<T>(command, args);
    } catch {
      return null;
    }
  }

  async function refreshAll(label = "Refreshing workspace") {
    setBusyLabel(label);
    try {
      const [
        snapshotResult,
        workflowResult,
        gitResult,
        actionsResult,
        historyResult,
        codeResult,
        tokenResult,
        modelResult,
        routingResult,
        settingsResult,
        dbResult,
        projConfigResult,
        tokenEstimatesResult,
        memoryResult,
        apiEnvDiagnosticResult,
        systemAgentsResult,
        systemCapabilitiesResult,
        systemPeripheralsResult,
        systemModulesResult,
      ] = await Promise.all([
        optionalCommand<unknown>("desktop_snapshot"),
        optionalCommand<unknown>("product_workflow_state"),
        optionalCommand<unknown>("git_workspace_snapshot"),
        optionalCommand<unknown>("desktop_actions"),
        optionalCommand<unknown[]>("action_history"),
        optionalCommand<unknown>("code_workbench_snapshot"),
        optionalCommand<TokenUsageSnapshot>("token_usage_snapshot"),
        optionalCommand<ModelHealthSnapshot>("model_health_snapshot"),
        optionalCommand<RoutingSnapshot>("routing_snapshot"),
        optionalCommand<ProviderSettings>("provider_settings"),
        optionalCommand<unknown>("db_status"),
        optionalCommand<any>("get_active_project_config"),
        optionalCommand<any[]>("get_project_file_token_estimates"),
        optionalCommand<string>("read_project_memory"),
        optionalCommand<ApiEnvDiagnostic>("get_api_env_diagnostic"),
        optionalCommand<AgentsConfig>("get_system_agents"),
        optionalCommand<CapabilitiesConfig>("get_system_capabilities"),
        optionalCommand<PeripheralsConfig>("get_system_peripherals"),
        optionalCommand<BrainModule[]>("get_system_modules"),
      ]);

      setSnapshot(snapshotResult);
      setWorkflow(workflowResult);
      setGit(gitResult);
      setActions(normalizeActions(actionsResult));
      setHistory(Array.isArray(historyResult) ? historyResult : []);
      setCodeWorkbench(codeResult);
      setTokens(tokenResult);
      setModels(modelResult);
      setRouting(routingResult);
      setApiEnvDiagnostic(apiEnvDiagnosticResult);
      setSystemAgents(systemAgentsResult);
      setSystemCapabilities(systemCapabilitiesResult);
      setSystemPeripherals(systemPeripheralsResult);
      setSystemModules(systemModulesResult || []);
      if (settingsResult) setProviderSettings(settingsResult);
      setDbState(dbResult);
      setProjectConfig(projConfigResult);
      if (tokenEstimatesResult) setFileTokenEstimates(tokenEstimatesResult);
      if (memoryResult) setProjectMemory(memoryResult);
      setBooting(false);
    } catch (error) {
      setBooting(false);
      pushToast("error", "Refresh failed", errorToMessage(error));
    } finally {
      setBusyLabel("");
    }
  }

  async function loadTokenEstimates() {
    setBusyLabel("Scanning workspace files");
    try {
      const result = await callCommand<any[]>("get_project_file_token_estimates");
      setFileTokenEstimates(result);
      pushToast("success", "Scan complete", `${result.length} text files scanned.`);
    } catch (error) {
      pushToast("error", "Scan failed", errorToMessage(error));
    } finally {
      setBusyLabel("");
    }
  }

  async function loadProjectMemory() {
    try {
      const result = await callCommand<string>("read_project_memory");
      setProjectMemory(result);
    } catch (error) {
      pushToast("error", "Memory load failed", errorToMessage(error));
    }
  }

  async function handleToggleIgnore(filePath: string) {
    if (!projectConfig) return;
    const currentIgnore = projectConfig.context_ignore || [];
    let nextIgnore: string[];
    if (currentIgnore.includes(filePath)) {
      nextIgnore = currentIgnore.filter((item: string) => item !== filePath);
      pushToast("info", "Removed ignore rule", filePath);
    } else {
      nextIgnore = [...currentIgnore, filePath];
      pushToast("success", "Added ignore rule", filePath);
    }
    setBusyLabel("Updating ignore rules");
    try {
      await callCommand("save_project_ignore_rules", { ignoreRules: nextIgnore });
      await refreshAll("Refreshing workspace after ignore change");
    } catch (error) {
      pushToast("error", "Failed to update ignore rules", errorToMessage(error));
    } finally {
      setBusyLabel("");
    }
  }

  async function handleRemoveIgnoreRule(rule: string) {
    if (!projectConfig) return;
    const nextIgnore = (projectConfig.context_ignore || []).filter((item: string) => item !== rule);
    setBusyLabel("Removing ignore rule");
    try {
      await callCommand("save_project_ignore_rules", { ignoreRules: nextIgnore });
      pushToast("info", "Removed ignore rule", rule);
      await refreshAll("Refreshing workspace after ignore change");
    } catch (error) {
      pushToast("error", "Failed to remove ignore rule", errorToMessage(error));
    } finally {
      setBusyLabel("");
    }
  }

  async function handleAppendMemory() {
    if (!memoryAppendInput.trim()) return;
    setBusyLabel("Saving memory log");
    try {
      await callCommand("append_project_memory", { content: memoryAppendInput.trim() });
      setMemoryAppendInput("");
      pushToast("success", "Memory log saved");
      const result = await callCommand<string>("read_project_memory");
      setProjectMemory(result);
      await refreshAll("Refreshing workspace after memory update");
    } catch (error) {
      pushToast("error", "Failed to save memory log", errorToMessage(error));
    } finally {
      setBusyLabel("");
    }
  }

  async function runAction(actionId: string) {
    setBusyLabel(`Running ${actionId}`);
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
    if (!hasProject || !hasTask) {
      pushToast("warning", "Setup required", "Connect a project and create a task first.");
      setActiveTab("settings");
      return;
    }
    if (!nextActionId) {
      pushToast("warning", "No next action", "Refresh workflow state or check Debug.");
      return;
    }
    if (dirty && ["judge", "agent", "patch"].some((word) => nextActionId.toLowerCase().includes(word))) {
      pushToast("warning", "Dirty workspace", "Review Git changes before agent-like actions.");
      setActiveTab("git");
      return;
    }
    await runAction(nextActionId);
  }

  async function addProjectFromSetup() {
    if (!setupForm.projectName.trim() || !setupForm.projectPath.trim()) {
      pushToast("warning", "Missing project data", "Project name and absolute path are required.");
      return;
    }
    setBusyLabel("Adding project");
    try {
      const input = {
        name: setupForm.projectName.trim(),
        path: setupForm.projectPath.trim(),
        project_type: setupForm.projectType.trim(),
        main_language: setupForm.mainLanguage.trim() || null,
      };
      const added = await callCommand<{ ok: boolean }>("project_add", { input });
      if (added.ok) await callCommand("project_use", { name: input.name });
      pushToast("success", "Project connected", input.name);
      await refreshAll("Refreshing project state");
    } catch (error) {
      pushToast("error", "Could not connect project", errorToMessage(error));
    } finally {
      setBusyLabel("");
    }
  }

  async function createTaskFromSetup() {
    if (!setupForm.taskTitle.trim()) {
      pushToast("warning", "Task title missing", "Add a clear task title.");
      return;
    }
    setBusyLabel("Creating task");
    try {
      await callCommand("task_new", { title: setupForm.taskTitle.trim() });
      pushToast("success", "Task created", setupForm.taskTitle.trim());
      await refreshAll("Refreshing task state");
      setActiveTab("workflow");
    } catch (error) {
      pushToast("error", "Could not create task", errorToMessage(error));
    } finally {
      setBusyLabel("");
    }
  }

  async function loadCodeFile(path: string) {
    setSelectedFile(path);
    setSelectedFileContent("Loading...");
    try {
      const result = await callCommand<unknown>("read_code_file", { relativePath: path, relative_path: path });
      setSelectedFileContent(getString(result, "content", stringifyPreview(result)));
    } catch (error) {
      setSelectedFileContent(errorToMessage(error));
      pushToast("error", "Could not read file", errorToMessage(error));
    }
  }

  async function loadArtifact(kind: string) {
    setArtifactKind(kind);
    setArtifactContent("Loading...");
    try {
      const result = await callCommand<unknown>("read_artifact", { kind });
      setArtifactContent(getString(result, "content", stringifyPreview(result)));
    } catch (error) {
      setArtifactContent(errorToMessage(error));
    }
  }

  async function logTokenUsage() {
    const inputTokens = Number.parseInt(tokenLogForm.inputTokens, 10);
    const outputTokens = Number.parseInt(tokenLogForm.outputTokens, 10);
    if (!Number.isFinite(inputTokens) || !Number.isFinite(outputTokens) || inputTokens < 0 || outputTokens < 0) {
      pushToast("warning", "Invalid token counts", "Use non-negative whole numbers.");
      return;
    }
    setBusyLabel("Logging token usage");
    try {
      const nextTokens = await callCommand<TokenUsageSnapshot>("log_token_usage", {
        input: {
          provider: tokenLogForm.provider.trim(),
          model: tokenLogForm.model.trim() || null,
          input_tokens: inputTokens,
          output_tokens: outputTokens,
          category: tokenLogForm.category.trim() || "general",
          notes: tokenLogForm.notes.trim() || null,
        },
      });
      setTokens(nextTokens);
      setTokenLogForm({ ...tokenLogForm, inputTokens: "0", outputTokens: "0", notes: "" });
      pushToast("success", "Token usage logged");
    } catch (error) {
      pushToast("error", "Could not log tokens", errorToMessage(error));
    } finally {
      setBusyLabel("");
    }
  }

  async function refreshModels() {
    setBusyLabel("Refreshing model health");
    try {
      const nextModels = await callCommand<ModelHealthSnapshot>("refresh_model_health");
      setModels(nextModels);
      pushToast("success", "Model health refreshed", `${nextModels.providers.filter((provider) => provider.reachability === "working").length} providers working`);
    } catch (error) {
      pushToast("error", "Could not refresh models", errorToMessage(error));
    } finally {
      setBusyLabel("");
    }
  }

  async function saveSettings() {
    setBusyLabel("Saving provider settings");
    try {
      const saved = await callCommand<ProviderSettings>("save_provider_settings", { input: providerSettings });
      setProviderSettings(saved);
      pushToast("success", "Provider settings saved");
      await refreshAll("Refreshing after settings");
    } catch (error) {
      pushToast("error", "Could not save settings", errorToMessage(error));
    } finally {
      setBusyLabel("");
    }
  }

  function renderBestRoutePanel() {
    const decision = routing?.decision;
    const request = routing?.request;
    const recommended = decision?.candidates.find((candidate) => candidate.provider === decision.recommended_provider);
    const candidateRows = decision?.candidates.slice(0, 5) ?? [];

    return (
      <section className="panel route-panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Best route</p>
            <h2>{decision ? `${decision.recommended_provider}${decision.recommended_model ? ` / ${decision.recommended_model}` : ""}` : "No route loaded"}</h2>
          </div>
          <span className={`pill ${statusTone(decision?.decision_level)}`}>{decision?.decision_level ?? "unknown"}</span>
        </div>

        {!decision ? (
          <p className="muted">Refresh workspace to calculate a route from current tokens, models, budget, and Git state.</p>
        ) : (
          <>
            <div className="route-summary-grid">
              <div><span>Task</span><strong>{request?.task_kind ?? decision.task_kind}</strong></div>
              <div><span>Score</span><strong>{decision.score}</strong></div>
              <div><span>Tokens</span><strong>{formatNumber(decision.estimated_total_tokens)}</strong></div>
              <div><span>Cost</span><strong>{formatCost(recommended?.estimated_cost_units, tokens?.cost_estimate.currency_label)}</strong></div>
              <div><span>Fallback</span><strong>{decision.fallback_provider ?? "manual"}</strong></div>
              <div><span>Files</span><strong>{formatNumber(request?.changed_file_count)}</strong></div>
            </div>

            {(decision.blockers.length > 0 || decision.warnings.length > 0 || decision.required_guardrails.length > 0) && (
              <div className="route-detail-grid">
                {decision.blockers.length > 0 && <RouteList title="Blockers" tone="danger" items={decision.blockers} />}
                {decision.warnings.length > 0 && <RouteList title="Warnings" tone="warn" items={decision.warnings} />}
                {decision.required_guardrails.length > 0 && <RouteList title="Guardrails" tone="ok" items={decision.required_guardrails} />}
              </div>
            )}

            <div className="route-candidate-strip">
              {candidateRows.map((candidate) => (
                <div className={`route-candidate ${candidate.blocked ? "blocked" : ""}`} key={candidate.provider}>
                  <strong>{candidate.provider}</strong>
                  <span>{candidate.kind} - {candidate.blocked ? "blocked" : `score ${candidate.score}`}</span>
                </div>
              ))}
            </div>
          </>
        )}
      </section>
    );
  }

  function renderDashboard() {
    const readiness = [
      { label: "Project", ok: hasProject, target: "settings" as TabId },
      { label: "Task", ok: hasTask, target: "settings" as TabId },
      { label: "Git", ok: Boolean(git), target: "git" as TabId },
      { label: "Tokens", ok: Boolean(tokens), target: "tokens" as TabId },
      { label: "Models", ok: Boolean(models), target: "models" as TabId },
    ];
    const readyCount = readiness.filter((item) => item.ok).length;
    return (
      <div className="content-grid dashboard-grid">
        {tokens && tokens.totals.total_tokens > 15000 && (
          <div className="notice warn wide-panel" style={{ display: "flex", justifyContent: "space-between", alignItems: "center", width: "100%", gridColumn: "1 / -1", gap: "12px" }}>
            <span>⚠️ <strong>Token Warning:</strong> Your current task context is heavy ({formatNumber(tokens.totals.total_tokens)} tokens). Review file token weights below to exclude large unnecessary files.</span>
            <button className="tiny-button" style={{ whiteSpace: "nowrap" }} onClick={() => setActiveTab("tokens")}>Open Token Advisor</button>
          </div>
        )}
        <section className="hero-panel wide-panel">
          <p className="eyebrow">RepoDesk cockpit</p>
          <h1>AI workflow state, tokens, and model health.</h1>
          <p className="lead">Use one screen to see the active project, next safe step, token usage, reachable models, and Git state before handing context to an agent.</p>
          <div className="button-row">
            <button className="primary-button" onClick={() => void doNextSafeStep()} disabled={isBusy}>{nextAction ? `Do next: ${nextAction.label}` : "Do next safe step"}</button>
            <button className="ghost-button" onClick={() => void refreshAll("Manual refresh")} disabled={isBusy}>Refresh</button>
          </div>
        </section>

        {renderBestRoutePanel()}

        <section className="panel">
          <div className="panel-title-row">
            <div><p className="eyebrow">Readiness</p><h2>{readyCount}/{readiness.length} ready</h2></div>
          </div>
          <div className="checklist-grid">
            {readiness.map((item) => (
              <button key={item.label} className={`check-card ${item.ok ? "ok" : "warn"}`} onClick={() => setActiveTab(item.target)}>
                <span>{item.ok ? "OK" : "Fix"}</span><strong>{item.label}</strong>
              </button>
            ))}
          </div>
        </section>

        <MetricCard label="Project" value={projectName} detail={`Task: ${taskTitle}`} />
        <MetricCard label="Git" value={dirty ? `${dirtyCount} changes` : "Clean"} detail={`Branch: ${branch}`} tone={dirty ? "warn" : "ok"} />
        <MetricCard label="Tokens" value={formatNumber(tokens?.totals.total_tokens)} detail={`${formatNumber(tokens?.totals.entries_count)} ledger entries`} />
        <MetricCard label="Models" value={`${modelCount} models`} detail={`${workingProviders} providers working`} tone={workingProviders ? "ok" : "warn"} />
      </div>
    );
  }

  function renderWorkflow() {
    const steps = asArray(getValue(workflow, "steps"));
    return (
      <div className="content-grid">
        <section className="hero-panel wide-panel">
          <p className="eyebrow">Workflow</p>
          <h1>{getString(workflow, "primary_cta", nextAction?.label ?? "One safe next step")}</h1>
          <p className="lead">{nextAction?.description ?? "Connect a project and task, then build bounded context before model usage."}</p>
          <div className="button-row">
            <button className="primary-button" onClick={() => void doNextSafeStep()} disabled={isBusy}>{nextAction?.label ?? "Do next safe step"}</button>
            <button className="ghost-button" onClick={() => void refreshAll("Refreshing workflow")} disabled={isBusy}>Refresh workflow</button>
          </div>
        </section>

        {renderBestRoutePanel()}

        <section className="panel wide-panel">
          <div className="timeline">
            {steps.length === 0 ? <p className="muted">No workflow steps loaded yet.</p> : steps.map((step, index) => {
              const record = asRecord(step);
              const status = getString(record, "status", "unknown");
              return (
                <div key={getString(record, "id", String(index))} className={`timeline-step ${statusTone(status)}`}>
                  <span>{index + 1}</span>
                  <strong>{getString(record, "title", `Step ${index + 1}`)}</strong>
                  <small>{status}</small>
                  <p>{getString(record, "description", "")}</p>
                </div>
              );
            })}
          </div>
        </section>

        <section className="panel">
          <p className="eyebrow">Next action</p>
          <h2>{nextAction?.label ?? "No action loaded"}</h2>
          <p className="muted">{nextAction?.description ?? "Open Settings to connect project and task."}</p>
          {dirty && <div className="notice warn">Workspace has Git changes. Review them before agent-like actions.</div>}
        </section>

        <section className="panel">
          <p className="eyebrow">Last result</p>
          <pre className="code-panel compact">{lastResult ? stringifyPreview(lastResult, 4000) : "No action has run in this session."}</pre>
        </section>
      </div>
    );
  }

  function renderTokens() {
    const providerRows = tokens?.by_provider ?? [];
    const modelRows = tokens?.by_model ?? [];
    return (
      <div className="content-grid">
        <section className="hero-panel wide-panel">
          <p className="eyebrow">Tokens</p>
          <h1>{formatNumber(tokens?.totals.total_tokens)} total tokens logged.</h1>
          <p className="lead">Track active artifact estimates, manual usage, and planning cost before sending context to local or paid models.</p>
          <div className="button-row">
            <button className="ghost-button" onClick={() => void refreshAll("Refreshing token usage")} disabled={isBusy}>Refresh tokens</button>
          </div>
        </section>

        <MetricCard label="Input" value={formatNumber(tokens?.totals.total_input_tokens)} detail="Logged input tokens" />
        <MetricCard label="Output" value={formatNumber(tokens?.totals.total_output_tokens)} detail="Logged output tokens" />
        <MetricCard label="Entries" value={formatNumber(tokens?.totals.entries_count)} detail="Ledger rows" />
        <MetricCard label="Estimated cost" value={formatCost(tokens?.cost_estimate.estimated_total_units, tokens?.cost_estimate.currency_label)} detail="Local planning units" />

        {tokens && (
          <section className="panel wide-panel" style={{ display: "grid", gap: "16px", padding: "20px" }}>
            <div className="panel-title-row" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div>
                <p className="eyebrow" style={{ margin: 0 }}>Daily Token Budget</p>
                <h2 style={{ margin: "4px 0 0 0", fontSize: "1.5rem" }}>Remaining Daily Budget</h2>
              </div>
              <span className="pill ok" style={{ fontSize: "14px", fontWeight: "bold", padding: "6px 12px", borderRadius: "6px" }}>
                {formatNumber(tokens.totals.remaining_daily_tokens)} tokens left
              </span>
            </div>
            
            <div style={{ display: "grid", gap: "8px" }}>
              <div style={{ display: "flex", justifyContent: "space-between", fontWeight: "500", fontSize: "14px" }}>
                <span>Today's Usage: <strong>{formatNumber(tokens.totals.today_total_tokens)}</strong> tokens</span>
                <span className="muted">Hard Limit: {formatNumber(tokens.totals.today_total_tokens + tokens.totals.remaining_daily_tokens)} tokens</span>
              </div>
              
              <div style={{ 
                width: "100%", 
                height: "12px", 
                backgroundColor: "rgba(255, 255, 255, 0.08)", 
                borderRadius: "6px", 
                overflow: "hidden",
                border: "1px solid var(--border)",
                marginTop: "4px"
              }}>
                <div style={{ 
                  width: `${Math.min(100, (tokens.totals.today_total_tokens / (tokens.totals.today_total_tokens + tokens.totals.remaining_daily_tokens || 1)) * 100)}%`, 
                  height: "100%", 
                  background: "linear-gradient(90deg, #10b981 0%, #3b82f6 100%)",
                  transition: "width 0.4s ease-in-out",
                  borderRadius: "6px"
                }} />
              </div>
              
              <div style={{ display: "flex", justifyContent: "space-between", fontSize: "12px", marginTop: "4px" }} className="muted">
                <span>{((tokens.totals.today_total_tokens / (tokens.totals.today_total_tokens + tokens.totals.remaining_daily_tokens || 1)) * 100).toFixed(1)}% used</span>
                <span>Resetting daily (UTC)</span>
              </div>
            </div>
          </section>
        )}

        <section className="panel wide-panel">
          <div className="panel-title-row"><div><p className="eyebrow">Active artifacts</p><h2>Context token estimates</h2></div></div>
          <div className="table-list">
            {(tokens?.active_artifacts ?? []).map((artifact) => (
              <div className="table-row" key={artifact.kind}>
                <div>
                  <strong>{artifact.title}</strong>
                  <span>{artifact.path || artifact.recommendation}</span>
                </div>
                <div className="row-meta">
                  <span className={`pill ${statusTone(artifact.status)}`}>{artifact.status}</span>
                  <strong>{artifact.exists ? formatNumber(artifact.estimated_tokens) : "-"}</strong>
                </div>
              </div>
            ))}
            {!tokens?.active_artifacts.length && <p className="muted">No active task artifacts yet.</p>}
          </div>
        </section>

        <section className="panel">
          <p className="eyebrow">By provider</p>
          <UsageRows rows={providerRows} empty="No provider usage logged yet." />
        </section>

        <section className="panel">
          <p className="eyebrow">By model</p>
          <UsageRows rows={modelRows} empty="No model-specific usage logged yet." />
        </section>

        <section className="panel wide-panel">
          <div className="panel-title-row">
            <div>
              <p className="eyebrow">Token Leak Advisor</p>
              <h2>Workspace token weights</h2>
            </div>
            <button className="tiny-button" onClick={() => void loadTokenEstimates()}>Scan files</button>
          </div>
          <p className="muted" style={{ marginBottom: "16px" }}>
            Identify files consuming the most tokens in your project. Click <strong>Ignore</strong> to exclude them from context packs and prevent token leaks.
          </p>
          <div className="table-list" style={{ maxHeight: "350px", overflowY: "auto", border: "1px solid var(--border)", borderRadius: "8px", padding: "8px" }}>
            {fileTokenEstimates.map((file) => {
              const isIgnored = projectConfig?.context_ignore?.includes(file.path) || false;
              return (
                <div className="table-row" key={file.path}>
                  <div style={{ display: "grid", gap: "4px" }}>
                    <strong><code>{file.path}</code></strong>
                    <span className="muted">{formatNumber(file.bytes)} bytes &bull; {file.status}</span>
                  </div>
                  <div className="row-meta">
                    <strong>{formatNumber(file.estimated_tokens)} tokens</strong>
                    <button 
                      className={`tiny-button ${isIgnored ? "active" : ""}`}
                      onClick={() => void handleToggleIgnore(file.path)}
                    >
                      {isIgnored ? "Ignored" : "Ignore"}
                    </button>
                  </div>
                </div>
              );
            })}
            {fileTokenEstimates.length === 0 && <p className="muted" style={{ padding: "12px", textAlign: "center" }}>No scanned text files. Click scan to analyze.</p>}
          </div>
        </section>

        <section className="panel wide-panel">
          <div className="panel-title-row">
            <div>
              <p className="eyebrow">Context ignore list</p>
              <h2>Active ignore rules</h2>
            </div>
          </div>
          <p className="muted" style={{ marginBottom: "14px" }}>
            These patterns are excluded from context compilation. Click <strong style={{ color: "var(--danger)" }}>&times;</strong> to remove a rule.
          </p>
          <div style={{ display: "flex", flexWrap: "wrap", gap: "8px" }}>
            {projectConfig?.context_ignore?.map((rule: string) => (
              <span key={rule} className="pill" style={{ display: "inline-flex", alignItems: "center", gap: "8px", padding: "6px 10px" }}>
                <code>{rule}</code>
                <button 
                  style={{ border: "none", background: "none", color: "var(--danger)", cursor: "pointer", padding: "0", fontWeight: "bold", fontSize: "14px" }}
                  onClick={() => void handleRemoveIgnoreRule(rule)}
                >
                  &times;
                </button>
              </span>
            ))}
            {(!projectConfig?.context_ignore || projectConfig.context_ignore.length === 0) && <p className="muted">No ignore rules configured.</p>}
          </div>
        </section>

        <section className="panel wide-panel">
          <div className="panel-title-row"><div><p className="eyebrow">Manual log</p><h2>Add token usage</h2></div><button className="primary-button" onClick={() => void logTokenUsage()} disabled={isBusy}>Log usage</button></div>
          <div className="form-grid">
            <label>Provider<input value={tokenLogForm.provider} onChange={(event) => setTokenLogForm({ ...tokenLogForm, provider: event.target.value })} /></label>
            <label>Model<input value={tokenLogForm.model} onChange={(event) => setTokenLogForm({ ...tokenLogForm, model: event.target.value })} placeholder="optional" /></label>
            <label>Input tokens<input inputMode="numeric" value={tokenLogForm.inputTokens} onChange={(event) => setTokenLogForm({ ...tokenLogForm, inputTokens: event.target.value })} /></label>
            <label>Output tokens<input inputMode="numeric" value={tokenLogForm.outputTokens} onChange={(event) => setTokenLogForm({ ...tokenLogForm, outputTokens: event.target.value })} /></label>
            <label>Category<input value={tokenLogForm.category} onChange={(event) => setTokenLogForm({ ...tokenLogForm, category: event.target.value })} /></label>
            <label className="span-2">Notes<textarea rows={3} value={tokenLogForm.notes} onChange={(event) => setTokenLogForm({ ...tokenLogForm, notes: event.target.value })} /></label>
          </div>
        </section>
      </div>
    );
  }

  function renderModels() {
    return (
      <div className="content-grid">
        <section className="hero-panel wide-panel">
          <p className="eyebrow">Models</p>
          <h1>{workingProviders} providers working, {modelCount} models visible.</h1>
          <p className="lead">RepoDesk checks local runtimes and enabled API providers live. API keys are read from environment variables only and are never displayed.</p>
          <div className="button-row">
            <button className="primary-button" onClick={() => void refreshModels()} disabled={isBusy}>Refresh model health</button>
            <button className="ghost-button" onClick={() => setActiveTab("settings")}>Provider settings</button>
          </div>
        </section>

        {(models?.warnings ?? []).map((warning) => <div className="notice warn wide-panel" key={warning}>{warning}</div>)}

        {(models?.providers ?? []).map((provider) => (
          <section className="panel provider-panel" key={provider.id}>
            <div className="panel-title-row">
              <div><p className="eyebrow">{provider.id}</p><h2>{provider.label}</h2></div>
              <span className={`pill ${statusTone(provider.reachability)}`}>{provider.reachability}</span>
            </div>
            <div className="provider-meta">
              <span>auth: {provider.auth_status}</span>
              <span>{provider.models.length} models</span>
            </div>
            {provider.error_summary && <div className="notice warn">{provider.error_summary}</div>}
            <div className="model-list">
              {provider.models.length === 0 ? <p className="muted">No models visible for this provider.</p> : provider.models.slice(0, 80).map((model) => (
                <div className="model-row" key={`${provider.id}-${model.id}`}>
                  <strong>{model.id}</strong>
                  {model.notes && <span>{model.notes}</span>}
                </div>
              ))}
            </div>
          </section>
        ))}
      </div>
    );
  }

  function renderCode() {
    const previews = asArray(asRecord(codeWorkbench).previews);
    return (
      <div className="content-grid code-grid">
        <section className="hero-panel wide-panel">
          <p className="eyebrow">Code</p>
          <h1>{changedFiles.length} changed files visible.</h1>
          <p className="lead">Inspect changed files before building prompts or asking an agent. Secret-like and binary paths stay blocked.</p>
          <div className="button-row">
            <button className="primary-button" onClick={() => void runAction("smart-context-build")} disabled={isBusy}>Build smart context</button>
            <button className="ghost-button" onClick={() => void refreshAll("Refreshing code workbench")} disabled={isBusy}>Refresh code</button>
          </div>
        </section>

        <section className="panel file-browser-panel">
          <div className="panel-title-row"><div><p className="eyebrow">Changed files</p><h2>Review before AI</h2></div><span className="pill">{changedFiles.length}</span></div>
          <div className="file-list scroll-area">
            {changedFiles.length === 0 ? <p className="muted">No changed files found or no active project connected.</p> : changedFiles.map((file) => (
              <button key={file} className={`file-row ${selectedFile === file ? "active" : ""}`} onClick={() => void loadCodeFile(file)}><code>{file}</code></button>
            ))}
          </div>
        </section>

        <section className="panel code-preview-panel">
          <div className="panel-title-row"><div><p className="eyebrow">File preview</p><h2>{selectedFile || "Select a file"}</h2></div>{selectedFileContent && <button className="tiny-button" onClick={() => void copyToClipboard(selectedFileContent).then(() => pushToast("success", "Copied", selectedFile))}>Copy</button>}</div>
          <pre className="code-panel tall">{selectedFileContent || "Pick a changed file to inspect safe preview."}</pre>
        </section>

        <section className="panel wide-panel">
          <div className="panel-title-row"><div><p className="eyebrow">Safe snippets</p><h2>Context candidates</h2></div><span className="pill">{previews.length}</span></div>
          <div className="snippet-grid">
            {previews.length === 0 ? <p className="muted">No previews yet.</p> : previews.slice(0, 8).map((item, index) => {
              const record = asRecord(item);
              return <div className="snippet-card" key={getString(record, "path", String(index))}><strong>{getString(record, "path", `file-${index}`)}</strong><span>{formatNumber(Number(getValue(record, "bytes") ?? 0))} bytes - {getString(record, "status", "changed")}</span></div>;
            })}
          </div>
        </section>
      </div>
    );
  }

  function renderGit() {
    const staged = listFromRecord(git, ["staged", "staged_files"]);
    const unstaged = listFromRecord(git, ["unstaged", "unstaged_files", "modified_files"]);
    const untracked = listFromRecord(git, ["untracked", "untracked_files"]);
    const diffStat = getString(git, "diff_stat", getString(git, "stat", "No diff stat available"));
    return (
      <div className="content-grid">
        <section className="hero-panel wide-panel">
          <p className="eyebrow">Git</p>
          <h1>{dirty ? `${dirtyCount} pending changes` : "Workspace clean"}</h1>
          <p className="lead">Read-only workspace view. RepoDesk does not stage, commit, reset, or push from this screen.</p>
          <button className="ghost-button" onClick={() => void refreshAll("Refreshing Git")} disabled={isBusy}>Refresh Git</button>
        </section>
        <MetricCard label="Branch" value={branch} detail={`Last commit: ${getString(git, "last_commit", "-")}`} />
        <MetricCard label="Staged" value={String(staged.length)} detail="Ready for commit" />
        <MetricCard label="Unstaged" value={String(unstaged.length)} detail="Modified but not staged" tone={unstaged.length ? "warn" : "ok"} />
        <MetricCard label="Untracked" value={String(untracked.length)} detail="New files" tone={untracked.length ? "warn" : "ok"} />
        <section className="panel wide-panel"><p className="eyebrow">Diff stat</p><pre className="code-panel">{diffStat}</pre></section>
        <FileGroup title="Staged files" files={staged} />
        <FileGroup title="Unstaged files" files={unstaged} />
        <FileGroup title="Untracked files" files={untracked} />
      </div>
    );
  }

  function renderSettings() {
    return (
      <div className="content-grid">
        <section className="hero-panel wide-panel">
          <p className="eyebrow">Settings</p>
          <h1>Project, task, and provider controls.</h1>
          <p className="lead">Provider settings store URLs, toggles, and environment variable names only. Raw API keys stay outside RepoDesk settings.</p>
          <div className="button-row">
            <button className="primary-button" onClick={() => void saveSettings()} disabled={isBusy}>Save provider settings</button>
            <button className="ghost-button" onClick={() => void refreshAll("Refreshing settings")} disabled={isBusy}>Refresh</button>
          </div>
        </section>

        <section className="panel">
          <p className="eyebrow">Connect project</p><h2>Active workspace</h2>
          <div className="form-stack">
            <label>Project name<input value={setupForm.projectName} onChange={(event) => setSetupForm({ ...setupForm, projectName: event.target.value })} /></label>
            <label>Project path<input value={setupForm.projectPath} onChange={(event) => setSetupForm({ ...setupForm, projectPath: event.target.value })} placeholder="/Users/mykyta/Documents/projects/repodesk" /></label>
            <label>Project type<input value={setupForm.projectType} onChange={(event) => setSetupForm({ ...setupForm, projectType: event.target.value })} /></label>
            <label>Main language<input value={setupForm.mainLanguage} onChange={(event) => setSetupForm({ ...setupForm, mainLanguage: event.target.value })} /></label>
            <button className="primary-button full" onClick={() => void addProjectFromSetup()} disabled={isBusy}>Add and activate project</button>
          </div>
        </section>

        <section className="panel">
          <p className="eyebrow">Task</p><h2>Create active task</h2>
          <div className="form-stack">
            <label>Task title<input value={setupForm.taskTitle} onChange={(event) => setSetupForm({ ...setupForm, taskTitle: event.target.value })} /></label>
            <button className="primary-button full" onClick={() => void createTaskFromSetup()} disabled={isBusy}>Create task</button>
          </div>
        </section>

        <section className="panel wide-panel">
          <div className="panel-title-row"><div><p className="eyebrow">Provider settings</p><h2>Runtime configuration</h2></div><span className={`pill ${statusTone(Boolean(dbState))}`}>DB {getString(dbState, "ok", "-")}</span></div>
          <div className="settings-grid">
            <Toggle label="Ollama enabled" checked={providerSettings.ollama_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, ollama_enabled: value })} />
            <Toggle label="LM Studio enabled" checked={providerSettings.lm_studio_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, lm_studio_enabled: value })} />
            <Toggle label="Llamafile enabled" checked={providerSettings.llamafile_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, llamafile_enabled: value })} />
            <Toggle label="LocalAI enabled" checked={providerSettings.localai_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, localai_enabled: value })} />
            <Toggle label="ChatGPT manual enabled" checked={providerSettings.chatgpt_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, chatgpt_enabled: value })} />
            <Toggle label="Codex enabled" checked={providerSettings.codex_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, codex_enabled: value })} />
            <Toggle label="Gemini manual enabled" checked={providerSettings.gemini_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, gemini_enabled: value })} />
            <Toggle label="OpenAI API enabled" checked={providerSettings.openai_api_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, openai_api_enabled: value })} />
            <Toggle label="Gemini API enabled" checked={providerSettings.gemini_api_enabled} onChange={(value) => setProviderSettings({ ...providerSettings, gemini_api_enabled: value })} />
            <Toggle label="Allow paid agents" checked={providerSettings.allow_paid_agents} onChange={(value) => setProviderSettings({ ...providerSettings, allow_paid_agents: value })} />
            <label>Codex quota proxy<select value={providerSettings.codex_quota_status} onChange={(event) => setProviderSettings({ ...providerSettings, codex_quota_status: event.target.value })}>
              <option value="unknown">unknown</option>
              <option value="available">available</option>
              <option value="limited">limited</option>
              <option value="empty">empty</option>
            </select></label>
            <label>Ollama URL<input value={providerSettings.ollama_url} onChange={(event) => setProviderSettings({ ...providerSettings, ollama_url: event.target.value })} /></label>
            <label>Ollama default model<input value={providerSettings.ollama_model} onChange={(event) => setProviderSettings({ ...providerSettings, ollama_model: event.target.value })} /></label>
            <label>LM Studio URL<input value={providerSettings.lm_studio_url} onChange={(event) => setProviderSettings({ ...providerSettings, lm_studio_url: event.target.value })} /></label>
            <label>Llamafile URL<input value={providerSettings.llamafile_url} onChange={(event) => setProviderSettings({ ...providerSettings, llamafile_url: event.target.value })} /></label>
            <label>LocalAI URL<input value={providerSettings.localai_url} onChange={(event) => setProviderSettings({ ...providerSettings, localai_url: event.target.value })} /></label>
            <label>OpenAI key env var<input value={providerSettings.openai_api_key_env_var} onChange={(event) => setProviderSettings({ ...providerSettings, openai_api_key_env_var: event.target.value })} /></label>
            <label>Gemini key env var<input value={providerSettings.gemini_api_key_env_var} onChange={(event) => setProviderSettings({ ...providerSettings, gemini_api_key_env_var: event.target.value })} /></label>
            <label>Patch provider<input value={providerSettings.preferred_patch_provider} onChange={(event) => setProviderSettings({ ...providerSettings, preferred_patch_provider: event.target.value })} /></label>
            <label>Compression provider<input value={providerSettings.preferred_compression_provider} onChange={(event) => setProviderSettings({ ...providerSettings, preferred_compression_provider: event.target.value })} /></label>
            <label>Review provider<input value={providerSettings.preferred_review_provider} onChange={(event) => setProviderSettings({ ...providerSettings, preferred_review_provider: event.target.value })} /></label>
            <label className="span-2">Notes<textarea rows={3} value={providerSettings.notes} onChange={(event) => setProviderSettings({ ...providerSettings, notes: event.target.value })} /></label>
          </div>
        </section>

        <section className="panel wide-panel">
          <div className="panel-title-row">
            <div>
              <p className="eyebrow">Project Memory & Guidelines</p>
              <h2>Active workspace instructions</h2>
            </div>
            <button className="tiny-button" onClick={() => void loadProjectMemory()}>Reload memory</button>
          </div>
          <p className="muted" style={{ marginBottom: "12px" }}>
            This memory is included in all context packs to guide external agents and avoid unwanted token usage on unnecessary directories or patterns.
          </p>
          <pre className="code-panel compact" style={{ whiteSpace: "pre-wrap", maxHeight: "250px", marginBottom: "14px", overflowY: "auto" }}>
            {projectMemory || "No guidelines or memory logs saved yet."}
          </pre>
          <div className="form-stack">
            <label>
              Add memory log / rule (e.g. "Do not change public API flags", "Always keep code modifications inside src-tauri/")
              <textarea 
                rows={3} 
                value={memoryAppendInput} 
                onChange={(event) => setMemoryAppendInput(event.target.value)} 
                placeholder="Guidelines, constraints, or architecture notes for agents to remember..."
              />
            </label>
            <button className="primary-button" onClick={() => void handleAppendMemory()} disabled={isBusy || !memoryAppendInput.trim()}>
              Add guidelines to memory.md
            </button>
          </div>
        </section>

        <section className="panel wide-panel" style={{ display: "grid", gap: "16px", padding: "20px" }}>
          <div className="panel-title-row">
            <div>
              <p className="eyebrow">Security & API Credentials</p>
              <h2>Secure API Environment Diagnostic</h2>
            </div>
          </div>
          <p className="muted">
            RepoDesk detects system environment variables to securely sign API requests without storing plaintext credentials in local files or databases.
          </p>

          <div style={{ display: "grid", gap: "12px", gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))" }}>
            <div className="card" style={{ 
              padding: "16px", 
              borderRadius: "8px", 
              border: "1px solid var(--border)", 
              backgroundColor: "rgba(255, 255, 255, 0.02)",
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between"
            }}>
              <div>
                <strong style={{ display: "block", fontSize: "14px", marginBottom: "4px" }}>OPENAI_API_KEY</strong>
                <span className="muted" style={{ fontSize: "12px" }}>For OpenAI GPT models and tools</span>
              </div>
              <span className={`pill ${apiEnvDiagnostic?.openai_api_key_set ? "ok" : "warn"}`} style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
                {apiEnvDiagnostic?.openai_api_key_set ? "🛡️ Securely Loaded" : "⚠️ Missing"}
              </span>
            </div>

            <div className="card" style={{ 
              padding: "16px", 
              borderRadius: "8px", 
              border: "1px solid var(--border)", 
              backgroundColor: "rgba(255, 255, 255, 0.02)",
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between"
            }}>
              <div>
                <strong style={{ display: "block", fontSize: "14px", marginBottom: "4px" }}>GEMINI_API_KEY</strong>
                <span className="muted" style={{ fontSize: "12px" }}>For Gemini reasoning and chat models</span>
              </div>
              <span className={`pill ${apiEnvDiagnostic?.gemini_api_key_set ? "ok" : "warn"}`} style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
                {apiEnvDiagnostic?.gemini_api_key_set ? "🛡️ Securely Loaded" : "⚠️ Missing"}
              </span>
            </div>

            <div className="card" style={{ 
              padding: "16px", 
              borderRadius: "8px", 
              border: "1px solid var(--border)", 
              backgroundColor: "rgba(255, 255, 255, 0.02)",
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between"
            }}>
              <div>
                <strong style={{ display: "block", fontSize: "14px", marginBottom: "4px" }}>ANTHROPIC_API_KEY</strong>
                <span className="muted" style={{ fontSize: "12px" }}>For Anthropic Claude models and agents</span>
              </div>
              <span className={`pill ${apiEnvDiagnostic?.anthropic_api_key_set ? "ok" : "warn"}`} style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
                {apiEnvDiagnostic?.anthropic_api_key_set ? "🛡️ Securely Loaded" : "⚠️ Missing"}
              </span>
            </div>
          </div>

          <div style={{ 
            marginTop: "8px", 
            padding: "16px", 
            borderRadius: "8px", 
            backgroundColor: "rgba(255, 255, 255, 0.04)", 
            borderLeft: "4px solid var(--border)" 
          }}>
            <strong style={{ display: "block", fontSize: "14px", marginBottom: "6px" }}>💡 How to configure environment variables permanently on macOS:</strong>
            <p className="muted" style={{ fontSize: "13px", margin: 0, lineHeight: "1.5" }}>
              To ensure RepoDesk and your terminal sessions can securely load credentials, add them to your shell config file (typically <code>~/.zshrc</code>). Run the following commands in your terminal:
            </p>
            <pre style={{ 
              backgroundColor: "rgba(0, 0, 0, 0.3)", 
              padding: "10px", 
              borderRadius: "4px", 
              fontSize: "12px", 
              margin: "10px 0 0 0", 
              fontFamily: "monospace", 
              overflowX: "auto" 
            }}>
{`echo 'export OPENAI_API_KEY="your-openai-key"' >> ~/.zshrc
echo 'export GEMINI_API_KEY="your-gemini-key"' >> ~/.zshrc
echo 'export ANTHROPIC_API_KEY="your-anthropic-key"' >> ~/.zshrc
source ~/.zshrc`}
            </pre>
          </div>
        </section>
      </div>
    );
  }

  function renderSystem() {
    return (
      <div className="content-grid">
        <section className="hero-panel wide-panel">
          <p className="eyebrow">Registry</p>
          <h1>Cognitive capabilities, MCP tools, and orchestrations.</h1>
          <p className="lead">Inspect all registered modules, peripheral access permissions, and model agents active on the RepoDesk platform.</p>
          <div className="button-row">
            <button className="ghost-button" onClick={() => void refreshAll("Refreshing registries")} disabled={isBusy}>Refresh system state</button>
          </div>
        </section>

        {/* Capabilities Panel */}
        <section className="panel wide-panel">
          <div className="panel-title-row">
            <div>
              <p className="eyebrow">Skills & Capabilities</p>
              <h2>Active Capabilities</h2>
            </div>
            <span className="pill ok">{systemCapabilities?.capabilities.filter(c => c.enabled).length ?? 0} enabled</span>
          </div>
          <div className="table-list">
            {(systemCapabilities?.capabilities ?? []).map((cap) => (
              <div className="table-row" key={cap.name} style={{ flexDirection: "column", alignItems: "flex-start", gap: "10px", padding: "16px" }}>
                <div style={{ display: "flex", justifyContent: "space-between", width: "100%", alignItems: "center" }}>
                  <div>
                    <strong style={{ fontSize: "16px" }}>{cap.name}</strong>
                    <span className="muted" style={{ fontSize: "12px", marginLeft: "12px" }}>kind: {cap.kind}</span>
                  </div>
                  <div style={{ display: "flex", gap: "8px" }}>
                    <span className={`pill ${cap.enabled ? "ok" : "disabled"}`}>{cap.enabled ? "Enabled" : "Disabled"}</span>
                    <span className={`pill ${cap.local ? "ok" : "warn"}`}>{cap.local ? "Local Only" : "Remote API"}</span>
                    <span className={`pill ${statusTone(cap.risk)}`}>{cap.risk} risk</span>
                  </div>
                </div>
                <div style={{ fontSize: "13px", color: "var(--text)", lineHeight: "1.4" }}>
                  <strong>Boundary:</strong> {cap.boundary}
                </div>
                {cap.allowed_actions.length > 0 && (
                  <div style={{ fontSize: "12px" }} className="muted">
                    <strong>Allowed:</strong> {cap.allowed_actions.join(", ")}
                  </div>
                )}
              </div>
            ))}
          </div>
        </section>

        {/* MCP & Peripherals Panel */}
        <section className="panel">
          <div className="panel-title-row">
            <div>
              <p className="eyebrow">MCP & Peripherals</p>
              <h2>Peripheral Tools</h2>
            </div>
          </div>
          <div className="table-list">
            {(systemPeripherals?.peripherals ?? []).map((p) => (
              <div className="table-row" key={p.name} style={{ flexDirection: "column", alignItems: "flex-start", gap: "6px", padding: "12px" }}>
                <div style={{ display: "flex", justifyContent: "space-between", width: "100%" }}>
                  <strong><code>{p.name}</code></strong>
                  <span className={`pill ${statusTone(p.risk)}`}>{p.risk} risk</span>
                </div>
                <span className="muted" style={{ fontSize: "12px" }}>Access: <strong>{p.access}</strong> &bull; Kind: {p.kind}</span>
                {p.allowed_actions.length > 0 && (
                  <div style={{ fontSize: "11px", marginTop: "4px" }} className="muted">
                    <strong>Allowed:</strong> {p.allowed_actions.join(", ")}
                  </div>
                )}
              </div>
            ))}
          </div>
        </section>

        {/* Orchestrator Agents Panel */}
        <section className="panel">
          <div className="panel-title-row">
            <div>
              <p className="eyebrow">Orchestration</p>
              <h2>Configured Agents</h2>
            </div>
          </div>
          <div className="table-list">
            {(systemAgents?.agents ?? []).map((agent) => (
              <div className="table-row" key={agent.name} style={{ flexDirection: "column", alignItems: "flex-start", gap: "6px", padding: "12px" }}>
                <div style={{ display: "flex", justifyContent: "space-between", width: "100%" }}>
                  <strong>{agent.name}</strong>
                  <span className="pill ok" style={{ fontSize: "11px" }}>{formatNumber(agent.default_budget_tokens)} token budget</span>
                </div>
                <p className="muted" style={{ fontSize: "12px", margin: 0 }}>{agent.role}</p>
                {agent.preferred_for.length > 0 && (
                  <div style={{ fontSize: "11px", marginTop: "4px" }} className="muted">
                    <strong>Preferred for:</strong> {agent.preferred_for.join(", ")}
                  </div>
                )}
              </div>
            ))}
          </div>
        </section>

        {/* Cognitive Modules Panel */}
        <section className="panel wide-panel">
          <div className="panel-title-row">
            <div>
              <p className="eyebrow">Brain Modules & Hooks</p>
              <h2>Core Cognitive Flow</h2>
            </div>
            <span className="pill ok">{systemModules.length} active layers</span>
          </div>
          <div style={{ display: "grid", gap: "12px", gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))" }}>
            {systemModules.map((module) => (
              <div key={module.name} style={{ 
                padding: "16px", 
                borderRadius: "8px", 
                border: "1px solid var(--border)", 
                backgroundColor: "rgba(255, 255, 255, 0.02)",
                display: "flex",
                flexDirection: "column",
                gap: "8px"
              }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <strong style={{ fontSize: "15px" }}>{module.name}</strong>
                  <span className={`pill ${module.status === "active" ? "ok" : "neutral"}`} style={{ fontSize: "11px" }}>{module.status}</span>
                </div>
                <span className="muted" style={{ fontSize: "11px" }}>Layer: <strong>{module.layer}</strong></span>
                <p style={{ fontSize: "12px", margin: 0, lineHeight: "1.4" }} className="muted">{module.purpose}</p>
              </div>
            ))}
          </div>
        </section>
      </div>
    );
  }

  function renderDebug() {
    return (
      <div className="content-grid">
        <section className="hero-panel wide-panel">
          <p className="eyebrow">Debug</p>
          <h1>{debugEvents.length} command traces.</h1>
          <p className="lead">Raw state lives here so the product screens stay focused.</p>
        </section>
        <section className="panel wide-panel">
          <div className="panel-title-row"><div><p className="eyebrow">Artifacts</p><h2>Prompt and context viewer</h2></div><button className="tiny-button" onClick={() => void loadArtifact(artifactKind)}>Load</button></div>
          <div className="button-row compact-buttons">
            {["context", "smart_context", "prompt_codex", "prompt_chatgpt", "prompt_review", "checks_summary", "token_estimate"].map((kind) => <button key={kind} className={artifactKind === kind ? "tiny-button active" : "tiny-button"} onClick={() => void loadArtifact(kind)}>{kind}</button>)}
          </div>
          <pre className="code-panel tall">{artifactContent || "Choose an artifact to preview."}</pre>
        </section>
        <section className="panel wide-panel">
          <p className="eyebrow">Command traces</p>
          <div className="debug-list">
            {debugEvents.map((event) => (
              <details key={event.id} className={`debug-event ${event.status}`}>
                <summary><strong>{event.command}</strong><span>{event.status}</span><small>{event.durationMs}ms - {event.timestamp}</small></summary>
                <pre>{event.error ?? event.preview ?? "No output."}</pre>
              </details>
            ))}
          </div>
        </section>
        <section className="panel wide-panel"><p className="eyebrow">Action history</p><pre className="code-panel tall">{history.length ? stringifyPreview(history, 8000) : "No action history yet."}</pre></section>
        <section className="panel wide-panel"><p className="eyebrow">Raw state</p><pre className="code-panel tall">{stringifyPreview({ snapshot, workflow, git, codeWorkbench, tokens, models, providerSettings, dbState }, 14000)}</pre></section>
      </div>
    );
  }

  function renderActiveTab() {
    if (booting) return <StartupSkeleton />;
    if (activeTab === "dashboard") return renderDashboard();
    if (activeTab === "workflow") return renderWorkflow();
    if (activeTab === "tokens") return renderTokens();
    if (activeTab === "models") return renderModels();
    if (activeTab === "code") return renderCode();
    if (activeTab === "git") return renderGit();
    if (activeTab === "settings") return renderSettings();
    if (activeTab === "system") return renderSystem();
    return renderDebug();
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand"><div className="brand-mark">RD</div><div><strong>RepoDesk</strong><span>AI control cockpit</span></div></div>
        <nav className="nav-list">
          {tabs.map((tab) => (
            <button key={tab.id} className={activeTab === tab.id ? "active" : ""} onClick={() => setActiveTab(tab.id)}>
              <strong>{tab.title}</strong><span>{tab.subtitle}</span>
            </button>
          ))}
        </nav>
      </aside>

      <main className="main-area">
        <header className="topbar">
          <div className="topbar-title"><p className="eyebrow">Active workspace</p><h2>{projectName}</h2></div>
          <div className="status-strip">
            <StatusBox label="Task" value={taskTitle} ok={hasTask} />
            <StatusBox label="Git" value={dirty ? `${dirtyCount} changes` : "Clean"} ok={!dirty} />
            <StatusBox label="Tokens" value={formatNumber(tokens?.totals.total_tokens)} ok={Boolean(tokens)} />
            <StatusBox label="Models" value={`${workingProviders}/${models?.providers.length ?? 0} working`} ok={workingProviders > 0} />
          </div>
        </header>
        {renderActiveTab()}
      </main>

      {isBusy && <div className="loading-overlay"><div className="loading-card"><div className="spinner" /><strong>{busyLabel}</strong><span>RepoDesk is working locally. Check Debug if it takes too long.</span></div></div>}
      <div className="toast-stack">{toasts.map((toast) => <div key={toast.id} className={`toast ${toast.kind}`}><strong>{toast.title}</strong>{toast.message && <span>{toast.message}</span>}</div>)}</div>
    </div>
  );
}

function StatusBox({ label, value, ok }: { label: string; value: string; ok: boolean }) {
  return (
    <div className={`status-box ${ok ? "ok" : "warn"}`} style={{ position: "relative" }}>
      <div style={{
        position: "absolute",
        top: "10px",
        right: "10px",
        width: "6px",
        height: "6px",
        borderRadius: "50%",
        backgroundColor: ok ? "var(--accent)" : "var(--warning)",
        boxShadow: ok ? "0 0 8px var(--accent)" : "0 0 8px var(--warning)"
      }} />
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function MetricCard({ label, value, detail, tone = "neutral" }: { label: string; value: string; detail: string; tone?: "neutral" | "ok" | "warn" }) {
  return <section className={`panel metric ${tone}`}><p className="eyebrow">{label}</p><h2>{value}</h2><p className="muted">{detail}</p></section>;
}

function UsageRows({ rows, empty }: { rows: TokenUsageItem[]; empty: string }) {
  if (rows.length === 0) return <p className="muted">{empty}</p>;
  return <div className="table-list">{rows.map((row) => (
    <div className="table-row" key={`${row.provider}-${row.model ?? "all"}`}>
      <div><strong>{row.model ? `${row.provider} / ${row.model}` : row.provider}</strong><span>{formatCost(row.estimated_cost_units, row.currency_label)}</span></div>
      <div className="row-meta"><span>in {formatNumber(row.input_tokens)}</span><span>out {formatNumber(row.output_tokens)}</span><strong>{formatNumber(row.total_tokens)}</strong></div>
    </div>
  ))}</div>;
}

function RouteList({ title, items, tone }: { title: string; items: string[]; tone: "ok" | "warn" | "danger" }) {
  return <div className={`route-list ${tone}`}><strong>{title}</strong>{items.slice(0, 4).map((item) => <span key={item}>{item}</span>)}</div>;
}

function FileGroup({ title, files }: { title: string; files: string[] }) {
  return <section className="panel"><div className="panel-title-row compact"><h2>{title}</h2><span className="pill">{files.length}</span></div><div className="file-list scroll-area small">{files.length ? files.map((file) => <code key={file}>{file}</code>) : <p className="muted">No files.</p>}</div></section>;
}

function Toggle({ label, checked, onChange }: { label: string; checked: boolean; onChange: (value: boolean) => void }) {
  return <label className="toggle-row"><input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} /><span>{label}</span></label>;
}

function StartupSkeleton() {
  return <div className="content-grid"><section className="hero-panel skeleton-panel"><div className="skeleton-line big" /><div className="skeleton-line" /><div className="skeleton-line short" /></section><section className="panel skeleton-panel"><div className="skeleton-line" /><div className="skeleton-line" /><div className="skeleton-line short" /></section><section className="panel skeleton-panel"><div className="skeleton-line" /><div className="skeleton-line short" /></section></div>;
}
