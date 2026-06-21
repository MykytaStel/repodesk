import { Suspense, useCallback, useEffect, useMemo, useState } from "react";
import { useQueryClient, useQuery } from "@tanstack/react-query";
import { getVersion } from "@tauri-apps/api/app";
import { callCommand } from "../shared/api/queries";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
import { EconomyMode } from "../features/routing/EconomyControl";
import { CommandPalette, type Command } from "../shared/ui/CommandPalette";
import { useToast } from "../shared/ui/Toast";
import { ProjectSwitcher } from "./ProjectSwitcher";
import { ThemeMenu } from "./ThemeMenu";
import { useWorkspace } from "../shared/hooks/useWorkspace";
import { useGit } from "../features/git/useGit";
import { useModels } from "../features/models/useModels";
import { useTokens } from "../features/tokens/useTokens";
import { errorToMessage, StartupSkeleton } from "../shared/ui/SharedComponents";
import { TabErrorBoundary } from "../shared/ui/TabErrorBoundary";
import type { TabId, Theme } from "../shared/types/api";
import { formatNumber } from "../shared/utils/helpers";
import { StatusBox } from "./StatusBox";
import { APP_TABS, PRIMARY_TABS, MORE_TABS, TAB_GROUP_ORDER, renderAppTab, type AppTab } from "./tabs";
import { STORAGE_KEYS } from "./constants";
import { readStoredActiveTab, readStoredEconomyMode, readStoredTheme } from "./storage";
import { BurgerIcon, ChevronIcon, TabIcon } from "./NavIcons";
import { TerminalPanel } from "../shared/ui/TerminalPanel";
import { ArtifactViewerModal } from "../shared/ui/ArtifactViewerModal";
import { AboutModal } from "../shared/ui/AboutModal";
import { GlobalLoader } from "../shared/ui/GlobalLoader";

const ABOUT_SEEN_KEY = "repodesk.about.seen";

type FeedbackTone = "info" | "success" | "error";
type ShellFeedback = {
  tone: FeedbackTone;
  title: string;
  detail: string;
};

/** One sidebar entry. Renders icon + label; when the rail is collapsed the
 * label is hidden via CSS and the title/subtitle move to a hover tooltip. */
function NavItem({
  tab,
  active,
  collapsed,
  onSelect,
}: {
  tab: AppTab;
  active: boolean;
  collapsed: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      className={`nav-item${active ? " active" : ""}`}
      onClick={onSelect}
      title={collapsed ? `${tab.title} — ${tab.subtitle}` : undefined}
      aria-label={collapsed ? tab.title : undefined}
    >
      <span className="nav-icon" aria-hidden="true">
        <TabIcon id={tab.id} />
      </span>
      <span className="nav-text">
        <strong>{tab.title}</strong>
        <span>{tab.subtitle}</span>
      </span>
    </button>
  );
}

export default function App() {
  const [activeTab, setActiveTab] = useState<TabId>(readStoredActiveTab);
  const [theme, setTheme] = useState<Theme>(readStoredTheme);
  const [economyMode, setEconomyMode] = useState<EconomyMode>(readStoredEconomyMode);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [terminalOpen, setTerminalOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [viewingArtifact, setViewingArtifact] = useState<string | null>(null);
  const [appVersion, setAppVersion] = useState("1.0.0");
  const [feedback, setFeedback] = useState<ShellFeedback | null>(null);
  // Collapsed sidebar (icon-only rail), persisted across sessions.
  const [collapsed, setCollapsed] = useState(
    () => window.localStorage.getItem(STORAGE_KEYS.sidebarCollapsed) === "1",
  );
  // Which Advanced group sections are expanded; the active deep tab's group
  // opens by default so the current surface is always visible in the list.
  const [openGroups, setOpenGroups] = useState<Record<string, boolean>>(() => {
    const active = MORE_TABS.find((tab) => tab.id === readStoredActiveTab());
    return active ? { [active.group]: true } : {};
  });
  const queryClient = useQueryClient();
  const toast = useToast();

  // React Query hooks driving the shell state
  const { projectName, taskTitle, hasProject, hasTask, isLoading: workspaceLoading } = useWorkspace();
  const { dirty, dirtyCount } = useGit();
  const { models } = useModels();
  const { tokens } = useTokens();

  const workingProviders = models?.providers?.filter((provider: any) => provider.reachability === "working").length ?? 0;
  const booting = workspaceLoading;
  const activeTabInfo = APP_TABS.find((tab) => tab.id === activeTab) ?? APP_TABS[0];

  const showFeedback = useCallback(
    (tone: FeedbackTone, title: string, detail: string, options?: { toast?: boolean }) => {
      setFeedback({ tone, title, detail });
      if (options?.toast === false) return;
      const message = detail ? `${title}: ${detail}` : title;
      if (tone === "success") toast.success(message);
      else if (tone === "error") toast.error(message);
      else toast.info(message);
    },
    [toast],
  );

  const navigateTo = useCallback(
    (tabId: TabId, detail?: string) => {
      const tab = APP_TABS.find((item) => item.id === tabId) ?? APP_TABS[0];
      setActiveTab(tabId);
      showFeedback("info", `Opened ${tab.title}`, detail ?? tab.subtitle, { toast: false });
    },
    [showFeedback],
  );

  const runWithFeedback = useCallback(
    async ({
      pending,
      success,
      task,
    }: {
      pending: string;
      success: string;
      task: () => Promise<void>;
    }) => {
      setFeedback({ tone: "info", title: pending, detail: "Running now." });
      toast.info(pending);
      try {
        await task();
        showFeedback("success", success, "Workspace data refreshed.");
      } catch (error) {
        showFeedback("error", "Action failed", errorToMessage(error));
      }
    },
    [showFeedback, toast],
  );

  useEffect(() => {
    let mounted = true;
    void getVersion()
      .then((version) => {
        if (mounted && typeof version === "string" && version.trim().length > 0) {
          setAppVersion(version);
        }
      })
      .catch(() => {
        // Browser-only test runs do not have Tauri IPC; keep the config fallback.
      });
    return () => {
      mounted = false;
    };
  }, []);

  const { data: projects = [] } = useQuery({
    queryKey: ["project_list_configs"],
    queryFn: () => invoke<any[]>("project_list_configs").catch(() => []),
    staleTime: 60_000,
  });


  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEYS.activeTab, activeTab);
    // Opening a deep tab expands its group so it stays visible in the list.
    const deep = MORE_TABS.find((tab) => tab.id === activeTab);
    if (deep) setOpenGroups((open) => ({ ...open, [deep.group]: true }));
  }, [activeTab]);

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEYS.sidebarCollapsed, collapsed ? "1" : "0");
  }, [collapsed]);

  // First run: surface the "What is RepoDesk?" explainer once for a brand-new
  // workspace (no project yet). It stays available from the header afterwards.
  useEffect(() => {
    if (booting || hasProject) return;
    if (window.localStorage.getItem(ABOUT_SEEN_KEY) === "1") return;
    window.localStorage.setItem(ABOUT_SEEN_KEY, "1");
    setAboutOpen(true);
  }, [booting, hasProject]);

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
      run: () => navigateTo(tab.id),
    }));
    const actions: Command[] = [
      {
        id: "action:refresh",
        label: "Refresh workspace",
        hint: "reload all data",
        run: () =>
          runWithFeedback({
            pending: "Refreshing workspace",
            success: "Workspace refreshed",
            task: async () => {
              await queryClient.invalidateQueries();
            },
          }),
      },
      {
        id: "action:generate-prompts",
        label: "Generate Prompts",
        hint: "trigger prompt-all agent",
        run: () =>
          runWithFeedback({
            pending: "Generating prompts",
            success: "Prompts generated",
            task: async () => {
              await callCommand("run_desktop_action", { actionId: "prompt-all" });
              await queryClient.invalidateQueries();
              setViewingArtifact("prompt_chatgpt");
            },
          }),
      },
      {
        id: "action:build-context",
        label: "Build Context",
        hint: "run context-build agent",
        run: () =>
          runWithFeedback({
            pending: "Building context",
            success: "Context built",
            task: async () => {
              await callCommand("run_desktop_action", { actionId: "context-build" });
              await queryClient.invalidateQueries();
              setViewingArtifact("agent_context_pack");
            },
          }),
      },
      {
        id: "action:run-checks",
        label: "Run Checks",
        hint: "run pre-commit checks",
        run: () =>
          runWithFeedback({
            pending: "Running checks",
            success: "Checks finished",
            task: async () => {
              await callCommand("run_desktop_action", { actionId: "checks-run" });
              await queryClient.invalidateQueries();
            },
          }),
      },
      {
        id: "action:theme-dark",
        label: "Theme: Dark",
        run: () => {
          setTheme("dark");
          showFeedback("success", "Theme changed", "Dark theme is active.");
        },
      },
      {
        id: "action:theme-light",
        label: "Theme: Light",
        run: () => {
          setTheme("light");
          showFeedback("success", "Theme changed", "Light theme is active.");
        },
      },
      {
        id: "action:theme-system",
        label: "Theme: Auto",
        run: () => {
          setTheme("system");
          showFeedback("success", "Theme changed", "System theme is active.");
        },
      },
    ];
    const projectCommands: Command[] = projects.map((p) => ({
      id: `project:${p.name}`,
      label: `Switch to project: ${p.name}`,
      hint: p.path,
      run: () => {
        return runWithFeedback({
          pending: `Switching to ${p.name}`,
          success: `Switched to ${p.name}`,
          task: async () => {
            await invoke("project_use", { name: p.name });
            await queryClient.invalidateQueries();
          },
        });
      }
    }));
    return [...tabCommands, ...actions, ...projectCommands];
  }, [navigateTo, projects, queryClient, runWithFeedback, showFeedback]);

  function renderActiveTab() {
    if (booting) return <StartupSkeleton />;
    const content = renderAppTab({ activeTab, economyMode, setActiveTab: navigateTo, setEconomyMode });
    return (
      <TabErrorBoundary tabId={activeTab}>
        <Suspense fallback={<StartupSkeleton />}>{content}</Suspense>
      </TabErrorBoundary>
    );
  }

  return (
    <div className={`app-shell${collapsed ? " app-shell--rail" : ""}`}>
      <GlobalLoader />
      <aside className={`sidebar${collapsed ? " sidebar--collapsed" : ""}`}>
        <div>
          <div className="brand">
            <div className="brand-mark">RD</div>
            <div className="brand-name"><strong>RepoDesk</strong><span>AI control cockpit</span></div>
            <button
              type="button"
              className="sidebar-burger"
              onClick={() => setCollapsed((value) => !value)}
              aria-pressed={collapsed}
              aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
              title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            >
              <BurgerIcon />
            </button>
          </div>
          <nav className="nav-list">
            <div className="nav-group">
              {PRIMARY_TABS.map((tab) => (
                <NavItem
                  key={tab.id}
                  tab={tab}
                  active={activeTab === tab.id}
                  collapsed={collapsed}
                  onSelect={() => navigateTo(tab.id)}
                />
              ))}
            </div>

            {collapsed ? (
              // Icon rail: a thin divider, then every deep tab as an icon.
              <div className="nav-group nav-rail-more">
                {MORE_TABS.map((tab) => (
                  <NavItem
                    key={tab.id}
                    tab={tab}
                    active={activeTab === tab.id}
                    collapsed
                    onSelect={() => navigateTo(tab.id)}
                  />
                ))}
              </div>
            ) : (
              // Expanded: three collapsible group sections (Work / AI / System).
              TAB_GROUP_ORDER.map((group) => {
                const tabs = MORE_TABS.filter((tab) => tab.group === group);
                if (tabs.length === 0) return null;
                const open = openGroups[group] ?? false;
                return (
                  <div key={group} className="nav-group">
                    <button
                      type="button"
                      className="nav-group-toggle"
                      aria-expanded={open}
                      onClick={() =>
                        setOpenGroups((prev) => ({ ...prev, [group]: !open }))
                      }
                    >
                      <span className="nav-caret" aria-hidden="true">
                        <ChevronIcon open={open} />
                      </span>
                      <span className="nav-group-name">{group}</span>
                      <span className="nav-group-count">{tabs.length}</span>
                    </button>
                    {open &&
                      tabs.map((tab) => (
                        <NavItem
                          key={tab.id}
                          tab={tab}
                          active={activeTab === tab.id}
                          collapsed={false}
                          onSelect={() => navigateTo(tab.id)}
                        />
                      ))}
                  </div>
                );
              })
            )}
          </nav>
        </div>
        <div className="sidebar-footer">
          {!collapsed && <ThemeMenu theme={theme} onChange={setTheme} />}
          <div
            className={`app-version${collapsed ? " app-version--rail" : ""}`}
            title={`RepoDesk v${appVersion}`}
            aria-label={`RepoDesk version ${appVersion}`}
          >
            v{appVersion}
          </div>
        </div>
      </aside>

      <main className="main-area">
        <header className="topbar">
          <div className="topbar-title">
            <p className="eyebrow">
              Active workspace
              <button
                type="button"
                className="link-button about-link"
                onClick={() => setAboutOpen(true)}
                title="What is RepoDesk?"
              >
                What is this?
              </button>
            </p>
            <ProjectSwitcher projectName={projectName} onConnectProject={() => navigateTo("settings", "Connect or edit a project.")} />
          </div>
          <div className="status-strip">
            <StatusBox label="Task" value={taskTitle} ok={hasTask} hint={hasTask ? "Active task — open the work flow" : "No active task — create one"} onClick={() => navigateTo("work", hasTask ? "Opened the current task flow." : "Create or select a task.")} />
            <StatusBox label="Git" value={dirty ? `${dirtyCount} changes` : "Clean"} ok={!dirty} hint={dirty ? "Uncommitted changes — review the diff" : "Working tree clean"} onClick={() => navigateTo("changes", dirty ? "Review changed files and diffs." : "Working tree is clean.")} />
            <StatusBox label="Tokens" value={formatNumber(tokens?.totals.total_tokens)} ok={Boolean(tokens)} hint="Token usage & cost" onClick={() => navigateTo("models-cost", "Opened usage and cost history.")} />
            <StatusBox label="Models" value={`${workingProviders}/${models?.providers.length ?? 0} working`} ok={workingProviders > 0} hint={workingProviders > 0 ? "Reachable models — open runtime health" : "No reachable models — configure providers"} onClick={() => navigateTo(workingProviders > 0 ? "models-cost" : "settings", workingProviders > 0 ? "Opened runtime health." : "Configure providers before running AI.")} />
            <StatusBox label="Terminal" value="Logs" ok={true} hint="Open system terminal" onClick={() => setTerminalOpen(true)} />
          </div>
        </header>
        <section className={`shell-feedback ${feedback?.tone ?? "neutral"}`} aria-live="polite">
          <div className="shell-feedback-view">
            <span className="shell-feedback-label">Active view</span>
            <strong>{activeTabInfo.title}</strong>
            <span>{activeTabInfo.subtitle}</span>
          </div>
          <div className="shell-feedback-last">
            <span className={`pill ${feedback?.tone === "success" ? "ok" : feedback?.tone === "error" ? "danger" : ""}`}>
              {feedback ? "Last action" : "Ready"}
            </span>
            <div>
              <strong>{feedback?.title ?? "No recent action"}</strong>
              <span>{feedback?.detail ?? "Workspace state is loaded."}</span>
            </div>
          </div>
        </section>
        {renderActiveTab()}
      </main>
      <AboutModal
        isOpen={aboutOpen}
        onClose={() => setAboutOpen(false)}
        onGetStarted={() => navigateTo(hasProject ? "work" : "settings", hasProject ? "Open the task flow." : "Connect a project to begin.")}
      />
      <TerminalPanel isOpen={terminalOpen} onClose={() => setTerminalOpen(false)} />
      <ArtifactViewerModal isOpen={!!viewingArtifact} kind={viewingArtifact || ""} onClose={() => setViewingArtifact(null)} />
      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} commands={commands} />
    </div>
  );
}
