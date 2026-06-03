import { useEffect, useState } from "react";
import "./App.css";
import { EconomyMode } from "../features/routing/EconomyControl";
import { useWorkspace } from "../shared/hooks/useWorkspace";
import { useGit } from "../features/git/useGit";
import { useModels } from "../features/models/useModels";
import { useTokens } from "../features/tokens/useTokens";
import { StartupSkeleton } from "../shared/ui/SharedComponents";
import { TabErrorBoundary } from "../shared/ui/TabErrorBoundary";
import type { TabId, Theme } from "../shared/types/api";
import { formatNumber } from "../shared/utils/helpers";
import { StatusBox } from "./StatusBox";
import { APP_TABS, renderAppTab } from "./tabs";
import { STORAGE_KEYS } from "./constants";
import { readStoredActiveTab, readStoredEconomyMode, readStoredTheme } from "./storage";

export default function App() {
  const [activeTab, setActiveTab] = useState<TabId>(readStoredActiveTab);
  const [theme, setTheme] = useState<Theme>(readStoredTheme);
  const [economyMode, setEconomyMode] = useState<EconomyMode>(readStoredEconomyMode);

  // React Query hooks driving the shell state
  const { projectName, taskTitle, hasProject, hasTask, isLoading: workspaceLoading } = useWorkspace();
  const { dirty, dirtyCount } = useGit();
  const { models } = useModels();
  const { tokens } = useTokens();

  const workingProviders = models?.providers?.filter((provider: any) => provider.reachability === "working").length ?? 0;
  const booting = workspaceLoading;


  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEYS.activeTab, activeTab);
  }, [activeTab]);

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEYS.economyMode, economyMode);
  }, [economyMode]);

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEYS.theme, theme);
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
    const content = renderAppTab({ activeTab, economyMode, setActiveTab, setEconomyMode });
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
            {APP_TABS.map((tab) => (
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
