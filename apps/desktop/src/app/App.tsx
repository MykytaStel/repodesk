import { useEffect, useState } from "react";
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
import { MemoryTab } from "../features/memory/MemoryTab";
import { useWorkspace } from "../shared/hooks/useWorkspace";
import { useGit } from "../features/git/useGit";
import { useWorkflow } from "../features/workflow/useWorkflow";
import { useCode } from "../features/code/useCode";
import { useModels } from "../features/models/useModels";
import { useTokens } from "../features/tokens/useTokens";
import { StartupSkeleton } from "../shared/ui/SharedComponents";
import { TabErrorBoundary } from "../shared/ui/TabErrorBoundary";
import type { TabId, Theme } from "../shared/types/api";
import { formatNumber } from "../shared/utils/helpers";

const tabs: Array<{ id: TabId; title: string; subtitle: string }> = [
  { id: "dashboard", title: "Dashboard", subtitle: "Daily state" },
  { id: "workflow", title: "Workflow", subtitle: "Next step" },
  { id: "tokens", title: "Tokens", subtitle: "Usage + cost" },
  { id: "models", title: "Models", subtitle: "Runtime health" },
  { id: "code", title: "Code", subtitle: "Changed files" },
  { id: "git", title: "Git", subtitle: "Workspace" },
  { id: "memory", title: "Memory", subtitle: "Project context" },
  { id: "settings", title: "Settings", subtitle: "Providers" },
  { id: "system", title: "System Registry", subtitle: "Skills & MCP" },
  { id: "debug", title: "Debug", subtitle: "Traces" },
];

export default function App() {
  const [activeTab, setActiveTab] = useState<TabId>(() => (window.localStorage.getItem("repodesk.activeTab") as TabId) || "dashboard");
  const [theme, setTheme] = useState<Theme>(() => (window.localStorage.getItem("repodesk.theme") as Theme) || "system");
  const [economyMode, setEconomyMode] = useState<EconomyMode>(() => (window.localStorage.getItem("repodesk.economyMode") as EconomyMode) || "balanced");

  // React Query hooks driving the shell state
  const { projectName, taskTitle, hasProject, hasTask, isLoading: workspaceLoading } = useWorkspace();
  const { git, dirty, dirtyCount, branch, isLoading: gitLoading } = useGit();
  const { workflow, nextAction, isLoading: workflowLoading } = useWorkflow();
  const { changedFiles, isLoading: codeLoading } = useCode();
  const { models, isLoading: modelsLoading } = useModels();
  const { tokens } = useTokens();

  const workingProviders = models?.providers?.filter((provider: any) => provider.reachability === "working").length ?? 0;
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
    let content: React.ReactNode;
    if (activeTab === "dashboard") {
      content = <DashboardTab setActiveTab={setActiveTab as any} economyMode={economyMode} setEconomyMode={setEconomyMode} />;
    } else if (activeTab === "workflow") {
      content = <WorkflowTab economyMode={economyMode} />;
    } else if (activeTab === "tokens") {
      content = <TokensTab />;
    } else if (activeTab === "models") {
      content = <ModelsTab setActiveTab={setActiveTab as any} />;
    } else if (activeTab === "code") {
      content = <CodeTab />;
    } else if (activeTab === "git") {
      content = <GitTab />;
    } else if (activeTab === "memory") {
      content = <MemoryTab />;
    } else if (activeTab === "settings") {
      content = <SettingsTab />;
    } else if (activeTab === "system") {
      content = <SystemTab />;
    } else {
      content = <DebugTab />;
    }
    return (
      <TabErrorBoundary tabId={activeTab}>
        {content}
      </TabErrorBoundary>
    );
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div>
          <div className="brand">
            <div className="brand-mark">RD</div>
            <div><strong>RepoDesk</strong><span>AI control cockpit</span></div>
          </div>
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
          <div className="topbar-title">
            <p className="eyebrow">Active workspace</p>
            <h2>{projectName}</h2>
          </div>
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
        boxShadow: ok ? "0 0 8px var(--accent)" : "0 0 8px var(--warning)",
      }} />
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
