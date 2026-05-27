import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import "./App.css";
import { DashboardTab } from "./components/DashboardTab";
import { WorkflowTab } from "./components/WorkflowTab";
import { TokensTab } from "./components/TokensTab";
import { ModelsTab } from "./components/ModelsTab";
import { CodeTab } from "./components/CodeTab";
import { GitTab } from "./components/GitTab";
import { SettingsTab } from "./components/SettingsTab";
import { SystemTab } from "./components/SystemTab";
import { DebugTab } from "./components/DebugTab";
import { StartupSkeleton } from "./components/SharedComponents";


type TabId = "dashboard" | "workflow" | "tokens" | "models" | "code" | "git" | "settings" | "system" | "debug";
type DebugStatus = "success" | "error";
type ToastKind = "success" | "error" | "warning" | "info";
type Theme = "dark" | "light" | "system";
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
  const [theme, setTheme] = useState<Theme>(() => (window.localStorage.getItem("repodesk.theme") as Theme) || "system");
  const [economyMode, setEconomyMode] = useState(() => window.localStorage.getItem("repodesk.economyMode") || "balanced");
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
  const [projectMemory, setProjectMemory] = useState<any[]>([]);
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
    window.localStorage.setItem("repodesk.economyMode", economyMode);
    // Refresh routing decision when economy mode changes
    if (!booting) void refreshAll("Updating economy routing");
  }, [economyMode]);

  useEffect(() => {
    window.localStorage.setItem("repodesk.theme", theme);
    const root = document.documentElement;
    
    const updateTheme = () => {
      if (theme === "system") {
        const isDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
        root.setAttribute("data-theme", isDark ? "dark" : "light");
      } else {
        root.setAttribute("data-theme", theme);
      }
    };

    updateTheme();
    
    if (theme === "system") {
      const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
      const handler = () => updateTheme();
      mediaQuery.addEventListener("change", handler);
      return () => mediaQuery.removeEventListener("change", handler);
    }
  }, [theme]);

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
        invoke<RoutingSnapshot>("routing_snapshot", { economyMode }).catch(() => null),
        optionalCommand<ProviderSettings>("provider_settings"),
        optionalCommand<unknown>("db_status"),
        optionalCommand<any>("get_active_project_config"),
        optionalCommand<any[]>("get_project_file_token_estimates"),
        optionalCommand<any[]>("memory_list", { project: projectName }),
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
    if (!projectName || projectName === "No active project" || projectName === "-") return;
    try {
      const result = await callCommand<any[]>("memory_list", { project: projectName });
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
    if (!projectName || projectName === "No active project" || projectName === "-") {
      pushToast("warning", "No project", "Connect a project before adding memory.");
      return;
    }
    if (!memoryAppendInput.trim()) return;
    setBusyLabel("Saving memory log");
    try {
      await callCommand("memory_add", { 
        project: projectName, 
        content: memoryAppendInput.trim(), 
        category: "general", 
        tags: [] 
      });
      setMemoryAppendInput("");
      pushToast("success", "Memory log saved");
      const result = await callCommand<any[]>("memory_list", { project: projectName });
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

  function renderActiveTab() {
    if (booting) return <StartupSkeleton />;
    if (activeTab === "dashboard") {
      return (
        <DashboardTab
          tokens={tokens}
          routing={routing}
          models={models}
          git={git}
          hasProject={hasProject}
          hasTask={hasTask}
          projectName={projectName}
          taskTitle={taskTitle}
          branch={branch}
          dirty={dirty}
          dirtyCount={dirtyCount}
          isBusy={isBusy}
          nextAction={nextAction}
          workingProviders={workingProviders}
          modelCount={modelCount}
          doNextSafeStep={doNextSafeStep}
          refreshAll={refreshAll}
          setActiveTab={setActiveTab}
          economyMode={economyMode}
          setEconomyMode={setEconomyMode}
        />
      );
    }
    if (activeTab === "workflow") {
      return (
        <WorkflowTab
          workflow={workflow}
          routing={routing}
          tokens={tokens}
          nextAction={nextAction}
          isBusy={isBusy}
          dirty={dirty}
          lastResult={lastResult}
          doNextSafeStep={doNextSafeStep}
          refreshAll={refreshAll}
        />
      );
    }
    if (activeTab === "tokens") {
      return (
        <TokensTab
          tokens={tokens}
          isBusy={isBusy}
          fileTokenEstimates={fileTokenEstimates}
          projectConfig={projectConfig}
          tokenLogForm={tokenLogForm}
          setTokenLogForm={setTokenLogForm}
          refreshAll={refreshAll}
          loadTokenEstimates={loadTokenEstimates}
          handleToggleIgnore={handleToggleIgnore}
          handleRemoveIgnoreRule={handleRemoveIgnoreRule}
          logTokenUsage={logTokenUsage}
        />
      );
    }
    if (activeTab === "models") {
      return (
        <ModelsTab
          models={models}
          workingProviders={workingProviders}
          modelCount={modelCount}
          isBusy={isBusy}
          refreshModels={refreshModels}
          setActiveTab={setActiveTab}
        />
      );
    }
    if (activeTab === "code") {
      return (
        <CodeTab
          codeWorkbench={codeWorkbench}
          changedFiles={changedFiles}
          isBusy={isBusy}
          selectedFile={selectedFile}
          selectedFileContent={selectedFileContent}
          runAction={runAction}
          refreshAll={refreshAll}
          loadCodeFile={loadCodeFile}
          pushToast={pushToast}
        />
      );
    }
    if (activeTab === "git") {
      return (
        <GitTab
          git={git}
          dirty={dirty}
          dirtyCount={dirtyCount}
          branch={branch}
          isBusy={isBusy}
          refreshAll={refreshAll}
        />
      );
    }
    if (activeTab === "settings") {
      return (
        <SettingsTab
          isBusy={isBusy}
          providerSettings={providerSettings}
          setProviderSettings={setProviderSettings}
          setupForm={setupForm}
          setSetupForm={setSetupForm}
          dbState={dbState}
          projectMemory={projectMemory}
          memoryAppendInput={memoryAppendInput}
          setMemoryAppendInput={setMemoryAppendInput}
          apiEnvDiagnostic={apiEnvDiagnostic}
          saveSettings={saveSettings}
          refreshAll={refreshAll}
          addProjectFromSetup={addProjectFromSetup}
          createTaskFromSetup={createTaskFromSetup}
          loadProjectMemory={loadProjectMemory}
          handleAppendMemory={handleAppendMemory}
        />
      );
    }
    if (activeTab === "system") {
      return (
        <SystemTab
          systemCapabilities={systemCapabilities}
          systemPeripherals={systemPeripherals}
          systemAgents={systemAgents}
          systemModules={systemModules}
          isBusy={isBusy}
          refreshAll={refreshAll}
        />
      );
    }
    return (
      <DebugTab
        debugEvents={debugEvents}
        artifactKind={artifactKind}
        artifactContent={artifactContent}
        history={history}
        loadArtifact={loadArtifact}
        snapshot={snapshot}
        workflow={workflow}
        git={git}
        codeWorkbench={codeWorkbench}
        tokens={tokens}
        models={models}
        providerSettings={providerSettings}
        dbState={dbState}
      />
    );
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div>
          <div className="brand"><div className="brand-mark">RD</div><div><strong>RepoDesk</strong><span>AI control cockpit</span></div></div>
          <nav className="nav-list">
            {tabs.map((tab) => (
              <button key={tab.id} className={activeTab === tab.id ? "active" : ""} onClick={() => setActiveTab(tab.id)}>
                <strong>{tab.title}</strong><span>{tab.subtitle}</span>
              </button>
            ))}
          </nav>
        </div>
        <div className="sidebar-footer">
          <p className="eyebrow" style={{marginBottom: 8}}>Theme</p>
          <div className="theme-switcher">
            <button className={theme === "light" ? "active" : ""} onClick={() => setTheme("light")}>Light</button>
            <button className={theme === "dark" ? "active" : ""} onClick={() => setTheme("dark")}>Dark</button>
            <button className={theme === "system" ? "active" : ""} onClick={() => setTheme("system")}>Auto</button>
          </div>
        </div>
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
