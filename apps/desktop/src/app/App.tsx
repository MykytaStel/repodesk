import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import "./App.css";
import { DashboardTab } from "../features/dashboard/DashboardTab";
import { EconomyMode } from "../features/routing/EconomyControl";
import { WorkflowTab } from "../features/workflow/WorkflowTab";
import { TokensTab } from "../features/tokens/TokensTab";
import { ModelsTab } from "../features/models/ModelsTab";
import { CodeTab } from "../features/code/CodeTab";
import { GitTab } from "../features/git/GitTab";
import { SettingsTab } from "../features/settings/SettingsTab";
import { SystemTab } from "../features/system/SystemTab";
import { DebugTab } from "../features/debug/DebugTab";
import { useWorkspace } from "../shared/hooks/useWorkspace";
import { useGit } from "../features/git/useGit";
import { useWorkflow } from "../features/workflow/useWorkflow";
import { useCode } from "../features/code/useCode";
import { useModels } from "../features/models/useModels";
import { useTokens } from "../features/tokens/useTokens";
import { StartupSkeleton } from "../shared/ui/SharedComponents";


type TabId = "dashboard" | "workflow" | "tokens" | "models" | "code" | "git" | "settings" | "system" | "debug";
type Theme = "dark" | "light" | "system";
type UnknownRecord = Record<string, unknown>;



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
  const [economyMode, setEconomyMode] = useState<EconomyMode>(() => (window.localStorage.getItem("repodesk.economyMode") as EconomyMode) || "balanced");

  // React Query Hooks driving the shell state
  const { snapshot, projectName, taskTitle, hasProject, hasTask, isLoading: workspaceLoading } = useWorkspace();
  const { git, dirty, dirtyCount, branch, isLoading: gitLoading } = useGit();
  const { workflow, nextAction, isLoading: workflowLoading } = useWorkflow();
  const { changedFiles, isLoading: codeLoading } = useCode();
  const { models, isLoading: modelsLoading } = useModels();
  const { tokens } = useTokens();

  const nextActionId = nextAction?.id;

  const workingProviders = models?.providers?.filter((provider: any) => provider.reachability === "working").length ?? 0;
  const modelCount = models?.providers?.reduce((total: number, provider: any) => total + provider.models.length, 0) ?? 0;
  
  const booting = workspaceLoading;

  useEffect(() => {
    window.localStorage.setItem("repodesk.activeTab", activeTab);
  }, [activeTab]);

  useEffect(() => {
    window.localStorage.setItem("repodesk.economyMode", economyMode);
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

  function renderActiveTab() {
    if (booting) return <StartupSkeleton />;
    if (activeTab === "dashboard") {
      return <DashboardTab setActiveTab={setActiveTab as any} economyMode={economyMode} setEconomyMode={setEconomyMode} />;
    }
    if (activeTab === "workflow") {
      return <WorkflowTab economyMode={economyMode} />;
    }
    if (activeTab === "tokens") {
      return <TokensTab />;
    }
    if (activeTab === "models") {
      return <ModelsTab setActiveTab={setActiveTab as any} />;
    }
    if (activeTab === "code") {
      return <CodeTab />;
    }
    if (activeTab === "git") {
      return <GitTab />;
    }
    if (activeTab === "settings") {
      return <SettingsTab />;
    }
    if (activeTab === "system") {
      return <SystemTab />;
    }
    return <DebugTab />;
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
          <p className="eyebrow" style={{ marginBottom: 8 }}>Theme</p>
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
