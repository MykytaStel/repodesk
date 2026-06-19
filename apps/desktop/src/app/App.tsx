import { Suspense, useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import "./App.css";
import { EconomyMode } from "../features/routing/EconomyControl";
import { CommandPalette, type Command } from "../shared/ui/CommandPalette";
import { ProjectSwitcher } from "./ProjectSwitcher";
import { ThemeMenu } from "./ThemeMenu";
import { useWorkspace } from "../shared/hooks/useWorkspace";
import { useGit } from "../features/git/useGit";
import { useModels } from "../features/models/useModels";
import { useTokens } from "../features/tokens/useTokens";
import { StartupSkeleton } from "../shared/ui/SharedComponents";
import { TabErrorBoundary } from "../shared/ui/TabErrorBoundary";
import type { TabId, Theme } from "../shared/types/api";
import { formatNumber } from "../shared/utils/helpers";
import { StatusBox } from "./StatusBox";
import { APP_TABS, TAB_GROUP_ORDER, renderAppTab } from "./tabs";
import { STORAGE_KEYS } from "./constants";
import { readStoredActiveTab, readStoredEconomyMode, readStoredTheme } from "./storage";

export default function App() {
  const [activeTab, setActiveTab] = useState<TabId>(readStoredActiveTab);
  const [theme, setTheme] = useState<Theme>(readStoredTheme);
  const [economyMode, setEconomyMode] = useState<EconomyMode>(readStoredEconomyMode);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const queryClient = useQueryClient();

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

  // Global keyboard shortcuts: ⌘K / Ctrl-K opens the palette; ⌘1..9 jump tabs.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen((open) => !open);
        return;
      }
      if (mod && /^[1-9]$/.test(e.key)) {
        const idx = Number(e.key) - 1;
        if (idx < APP_TABS.length) {
          e.preventDefault();
          setActiveTab(APP_TABS[idx].id);
        }
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const commands = useMemo<Command[]>(() => {
    const tabCommands: Command[] = APP_TABS.map((tab) => ({
      id: `goto:${tab.id}`,
      label: `Go to ${tab.title}`,
      hint: tab.subtitle,
      run: () => setActiveTab(tab.id),
    }));
    const actions: Command[] = [
      { id: "action:refresh", label: "Refresh workspace", hint: "reload all data", run: () => void queryClient.invalidateQueries() },
      { id: "action:theme-dark", label: "Theme: Dark", run: () => setTheme("dark") },
      { id: "action:theme-light", label: "Theme: Light", run: () => setTheme("light") },
      { id: "action:theme-system", label: "Theme: Auto", run: () => setTheme("system") },
    ];
    return [...tabCommands, ...actions];
  }, [queryClient]);

  function renderActiveTab() {
    if (booting) return <StartupSkeleton />;
    const content = renderAppTab({ activeTab, economyMode, setActiveTab, setEconomyMode });
    return (
      <TabErrorBoundary tabId={activeTab}>
        <Suspense fallback={<StartupSkeleton />}>{content}</Suspense>
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
            {TAB_GROUP_ORDER.map((group) => (
              <div key={group} className="nav-group">
                <p className="nav-group-title">{group}</p>
                {APP_TABS.filter((tab) => tab.group === group).map((tab) => (
                  <button key={tab.id} className={activeTab === tab.id ? "active" : ""} onClick={() => setActiveTab(tab.id)}>
                    <strong>{tab.title}</strong><span>{tab.subtitle}</span>
                  </button>
                ))}
              </div>
            ))}
          </nav>
        </div>
        <div className="sidebar-footer">
          <ThemeMenu theme={theme} onChange={setTheme} />
        </div>
      </aside>

      <main className="main-area">
        <header className="topbar">
          <div className="topbar-title">
            <p className="eyebrow">Active workspace</p>
            <ProjectSwitcher projectName={projectName} onConnectProject={() => setActiveTab("settings")} />
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
      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} commands={commands} />
    </div>
  );
}
