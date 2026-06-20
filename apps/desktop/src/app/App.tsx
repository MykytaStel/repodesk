import { Suspense, useEffect, useMemo, useState } from "react";
import { useQueryClient, useQuery } from "@tanstack/react-query";
import { callCommand } from "../shared/api/queries";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
import { APP_TABS, PRIMARY_TABS, MORE_TABS, TAB_GROUP_ORDER, renderAppTab } from "./tabs";
import { STORAGE_KEYS } from "./constants";
import { readStoredActiveTab, readStoredEconomyMode, readStoredTheme } from "./storage";

export default function App() {
  const [activeTab, setActiveTab] = useState<TabId>(readStoredActiveTab);
  const [theme, setTheme] = useState<Theme>(readStoredTheme);
  const [economyMode, setEconomyMode] = useState<EconomyMode>(readStoredEconomyMode);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [moreOpen, setMoreOpen] = useState(() => MORE_TABS.some((tab) => tab.id === readStoredActiveTab()));
  const queryClient = useQueryClient();

  // React Query hooks driving the shell state
  const { projectName, taskTitle, hasProject, hasTask, isLoading: workspaceLoading } = useWorkspace();
  const { dirty, dirtyCount } = useGit();
  const { models } = useModels();
  const { tokens } = useTokens();

  const workingProviders = models?.providers?.filter((provider: any) => provider.reachability === "working").length ?? 0;
  const booting = workspaceLoading;

  const { data: projects = [] } = useQuery({
    queryKey: ["project_list_configs"],
    queryFn: () => invoke<any[]>("project_list_configs").catch(() => []),
    staleTime: 60_000,
  });


  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEYS.activeTab, activeTab);
    if (MORE_TABS.some((tab) => tab.id === activeTab)) setMoreOpen(true);
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
    
    // Listen for global shortcut trigger from Tauri
    const unlisten = listen('open-command-palette', () => {
      setPaletteOpen(true);
    });

    return () => {
      window.removeEventListener("keydown", handler);
      unlisten.then(f => f());
    };
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
      { id: "action:generate-prompts", label: "Generate Prompts", hint: "trigger prompt-all agent", run: () => void callCommand("run_desktop_action", { actionId: "prompt-all" }).then(() => queryClient.invalidateQueries()) },
      { id: "action:build-context", label: "Build Context", hint: "run context-build agent", run: () => void callCommand("run_desktop_action", { actionId: "context-build" }).then(() => queryClient.invalidateQueries()) },
      { id: "action:run-checks", label: "Run Checks", hint: "run pre-commit checks", run: () => void callCommand("run_desktop_action", { actionId: "checks-run" }).then(() => queryClient.invalidateQueries()) },
      { id: "action:theme-dark", label: "Theme: Dark", run: () => setTheme("dark") },
      { id: "action:theme-light", label: "Theme: Light", run: () => setTheme("light") },
      { id: "action:theme-system", label: "Theme: Auto", run: () => setTheme("system") },
    ];
    const projectCommands: Command[] = projects.map((p) => ({
      id: `project:${p.name}`,
      label: `Switch to project: ${p.name}`,
      hint: p.path,
      run: () => {
        void invoke("project_use", { name: p.name })
          .then(() => queryClient.invalidateQueries())
          .catch((e: any) => console.error("Failed to switch project", e));
      }
    }));
    return [...tabCommands, ...actions, ...projectCommands];
  }, [queryClient, projects]);

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
            <div className="nav-group">
              {PRIMARY_TABS.map((tab) => (
                <button key={tab.id} className={activeTab === tab.id ? "active" : ""} onClick={() => setActiveTab(tab.id)}>
                  <strong>{tab.title}</strong><span>{tab.subtitle}</span>
                </button>
              ))}
            </div>
            <div className="nav-group">
              <button type="button" className="nav-more-toggle" onClick={() => setMoreOpen((open) => !open)} aria-expanded={moreOpen}>
                <strong>More</strong><span>{moreOpen ? "Hide depth & diagnostics ▾" : "Depth & diagnostics ▸"}</span>
              </button>
              {moreOpen && TAB_GROUP_ORDER.map((group) => {
                const tabs = MORE_TABS.filter((tab) => tab.group === group);
                if (tabs.length === 0) return null;
                return (
                  <div key={group} className="nav-subgroup">
                    <p className="nav-group-title">{group}</p>
                    {tabs.map((tab) => (
                      <button key={tab.id} className={activeTab === tab.id ? "active" : ""} onClick={() => setActiveTab(tab.id)}>
                        <strong>{tab.title}</strong><span>{tab.subtitle}</span>
                      </button>
                    ))}
                  </div>
                );
              })}
            </div>
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
            <StatusBox label="Task" value={taskTitle} ok={hasTask} hint={hasTask ? "Active task — open the workflow" : "No active task — create one"} onClick={() => setActiveTab("workflow")} />
            <StatusBox label="Git" value={dirty ? `${dirtyCount} changes` : "Clean"} ok={!dirty} hint={dirty ? "Uncommitted changes — review the diff" : "Working tree clean"} onClick={() => setActiveTab("git")} />
            <StatusBox label="Tokens" value={formatNumber(tokens?.totals.total_tokens)} ok={Boolean(tokens)} hint="Token usage & cost" onClick={() => setActiveTab("tokens")} />
            <StatusBox label="Models" value={`${workingProviders}/${models?.providers.length ?? 0} working`} ok={workingProviders > 0} hint={workingProviders > 0 ? "Reachable models — open runtime health" : "No reachable models — configure providers"} onClick={() => setActiveTab(workingProviders > 0 ? "models" : "settings")} />
          </div>
        </header>
        {renderActiveTab()}
      </main>
      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} commands={commands} />
    </div>
  );
}
