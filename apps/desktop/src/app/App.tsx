import { Suspense, useCallback, useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
import { EconomyMode } from "../features/routing/EconomyControl";
import { useGit } from "../features/git/useGit";
import { useModels } from "../features/models/useModels";
import { useTokens } from "../features/tokens/useTokens";
import { callCommand } from "../shared/api/queries";
import { useWorkspace } from "../shared/hooks/useWorkspace";
import type { TabId, Theme } from "../shared/types/api";
import { AboutModal } from "../shared/ui/AboutModal";
import { ArtifactViewerModal } from "../shared/ui/ArtifactViewerModal";
import { CommandPalette, type Command } from "../shared/ui/CommandPalette";
import { GlobalLoader } from "../shared/ui/GlobalLoader";
import { errorToMessage, StartupSkeleton } from "../shared/ui/SharedComponents";
import { TabErrorBoundary } from "../shared/ui/TabErrorBoundary";
import { useToast } from "../shared/ui/Toast";
import { formatNumber } from "../shared/utils/helpers";
import { ActivityRail } from "./ActivityRail";
import { STORAGE_KEYS } from "./constants";
import { readStoredActiveTab, readStoredEconomyMode, readStoredTheme } from "./storage";
import { APP_TABS, PRIMARY_TABS, renderAppTab } from "./tabs";
import { WorkbenchBottomPanel } from "./WorkbenchBottomPanel";
import { WorkspaceInspector } from "./WorkspaceInspector";
import { WorkspaceSidebar } from "./WorkspaceSidebar";

const ABOUT_SEEN_KEY = "repodesk.about.seen";

type FeedbackTone = "info" | "success" | "error";
type ShellFeedback = {
  tone: FeedbackTone;
  title: string;
  detail: string;
};

type ProjectCommandTarget = {
  name: string;
  path: string;
};

export default function App() {
  const [activeTab, setActiveTab] = useState<TabId>(readStoredActiveTab);
  const [theme, setTheme] = useState<Theme>(readStoredTheme);
  const [economyMode, setEconomyMode] = useState<EconomyMode>(readStoredEconomyMode);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [viewingArtifact, setViewingArtifact] = useState<string | null>(null);
  const [appVersion, setAppVersion] = useState("1.0.0");
  const [feedback, setFeedback] = useState<ShellFeedback | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(
    () => window.localStorage.getItem(STORAGE_KEYS.sidebarCollapsed) !== "1",
  );
  const [inspectorOpen, setInspectorOpen] = useState(
    () => window.localStorage.getItem(STORAGE_KEYS.inspectorOpen) !== "0",
  );
  const [bottomPanelOpen, setBottomPanelOpen] = useState(
    () => window.localStorage.getItem(STORAGE_KEYS.bottomPanelOpen) === "1",
  );

  const queryClient = useQueryClient();
  const toast = useToast();

  const {
    projectName,
    taskTitle,
    hasProject,
    hasTask,
    isLoading: workspaceLoading,
  } = useWorkspace();
  const { dirty, dirtyCount } = useGit();
  const { models } = useModels();
  const { tokens } = useTokens();

  const workingProviders =
    models?.providers?.filter((provider: { reachability?: string }) => provider.reachability === "working").length ?? 0;
  const providerCount = models?.providers?.length ?? 0;
  const totalTokens = tokens?.totals.total_tokens;
  const booting = workspaceLoading;
  const activeTabInfo = APP_TABS.find((tab) => tab.id === activeTab) ?? APP_TABS[0];

  const { data: projects = [] } = useQuery({
    queryKey: ["project_list_configs"],
    queryFn: () => invoke<ProjectCommandTarget[]>("project_list_configs").catch(() => []),
    staleTime: 60_000,
  });

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
      setActiveTab(tab.id);
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
        // Browser-only rendering has no Tauri IPC; keep the config fallback.
      });
    return () => {
      mounted = false;
    };
  }, []);

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEYS.activeTab, activeTab);
  }, [activeTab]);

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEYS.sidebarCollapsed, sidebarOpen ? "0" : "1");
  }, [sidebarOpen]);

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEYS.inspectorOpen, inspectorOpen ? "1" : "0");
  }, [inspectorOpen]);

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEYS.bottomPanelOpen, bottomPanelOpen ? "1" : "0");
  }, [bottomPanelOpen]);

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

  // IDE shortcuts: command palette, primary activity surfaces, sidebar and panel.
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      const mod = event.metaKey || event.ctrlKey;
      if (!mod) return;

      const key = event.key.toLowerCase();
      if (key === "k") {
        event.preventDefault();
        setPaletteOpen((open) => !open);
        return;
      }
      if (key === "b") {
        event.preventDefault();
        setSidebarOpen((open) => !open);
        return;
      }
      if (key === "j") {
        event.preventDefault();
        setBottomPanelOpen((open) => !open);
        return;
      }
      if (/^[1-9]$/.test(event.key)) {
        const index = Number(event.key) - 1;
        if (index < PRIMARY_TABS.length) {
          event.preventDefault();
          setActiveTab(PRIMARY_TABS[index].id);
        }
      }
    };

    window.addEventListener("keydown", handler);
    const unlisten = listen("open-command-palette", () => setPaletteOpen(true));

    return () => {
      window.removeEventListener("keydown", handler);
      void unlisten.then((dispose) => dispose());
    };
  }, []);

  const commands = useMemo<Command[]>(() => {
    const tabCommands: Command[] = APP_TABS.map((tab) => ({
      id: `goto:${tab.id}`,
      label: `Go to ${tab.title}`,
      hint: tab.subtitle,
      run: () => navigateTo(tab.id),
    }));

    const shellCommands: Command[] = [
      {
        id: "shell:sidebar",
        label: "Toggle workspace sidebar",
        hint: "⌘/Ctrl+B",
        run: () => setSidebarOpen((open) => !open),
      },
      {
        id: "shell:inspector",
        label: "Toggle inspector",
        run: () => setInspectorOpen((open) => !open),
      },
      {
        id: "shell:bottom-panel",
        label: "Toggle bottom panel",
        hint: "⌘/Ctrl+J",
        run: () => setBottomPanelOpen((open) => !open),
      },
    ];

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
        hint: "prepare bounded Work Item context",
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
        hint: "run configured verification checks",
        run: () =>
          runWithFeedback({
            pending: "Running checks",
            success: "Checks finished",
            task: async () => {
              await callCommand("run_desktop_action", { actionId: "checks-run" });
              await queryClient.invalidateQueries();
              setBottomPanelOpen(true);
            },
          }),
      },
      {
        id: "action:theme-dark",
        label: "Theme: Dark",
        run: () => setTheme("dark"),
      },
      {
        id: "action:theme-light",
        label: "Theme: Light",
        run: () => setTheme("light"),
      },
      {
        id: "action:theme-system",
        label: "Theme: Auto",
        run: () => setTheme("system"),
      },
    ];

    const projectCommands: Command[] = projects.map((project) => ({
      id: `project:${project.name}`,
      label: `Switch to project: ${project.name}`,
      hint: project.path,
      run: () =>
        runWithFeedback({
          pending: `Switching to ${project.name}`,
          success: `Switched to ${project.name}`,
          task: async () => {
            await invoke("project_use", { name: project.name });
            await queryClient.invalidateQueries();
          },
        }),
    }));

    return [...tabCommands, ...shellCommands, ...actions, ...projectCommands];
  }, [navigateTo, projects, queryClient, runWithFeedback]);

  function renderActiveTab() {
    if (booting) return <StartupSkeleton />;
    const content = renderAppTab({
      activeTab,
      economyMode,
      setActiveTab: navigateTo,
      setEconomyMode,
    });
    return (
      <TabErrorBoundary tabId={activeTab}>
        <Suspense fallback={<StartupSkeleton />}>{content}</Suspense>
      </TabErrorBoundary>
    );
  }

  const shellClassName = [
    "ide-shell",
    !sidebarOpen ? "no-context-sidebar" : "",
    !inspectorOpen ? "no-inspector" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={shellClassName}>
      <GlobalLoader />

      <ActivityRail
        activeTab={activeTab}
        tabs={PRIMARY_TABS}
        sidebarOpen={sidebarOpen}
        inspectorOpen={inspectorOpen}
        bottomPanelOpen={bottomPanelOpen}
        appVersion={appVersion}
        onSelect={navigateTo}
        onToggleSidebar={() => setSidebarOpen((open) => !open)}
        onToggleInspector={() => setInspectorOpen((open) => !open)}
        onToggleBottomPanel={() => setBottomPanelOpen((open) => !open)}
        onOpenPalette={() => setPaletteOpen(true)}
      />

      {sidebarOpen ? (
        <WorkspaceSidebar
          activeTab={activeTab}
          activeTabInfo={activeTabInfo}
          projectName={projectName ?? ""}
          taskTitle={taskTitle ?? ""}
          hasProject={hasProject}
          hasTask={hasTask}
          dirty={dirty}
          dirtyCount={dirtyCount}
          theme={theme}
          onThemeChange={setTheme}
          onNavigate={navigateTo}
        />
      ) : null}

      <section className="ide-workbench">
        <header className="ide-titlebar">
          <div className="ide-breadcrumbs" aria-label="Current workspace location">
            <strong>{projectName || "No project"}</strong>
            <span>/</span>
            <span>{activeTabInfo.title}</span>
            {hasTask ? <><span>/</span><small>{taskTitle}</small></> : null}
          </div>

          <div className="ide-titlebar-actions">
            <button type="button" className={`ide-status-chip${dirty ? " warning" : " ok"}`} onClick={() => navigateTo("changes")}>
              Git {dirty ? dirtyCount : "clean"}
            </button>
            <button type="button" className={`ide-status-chip${workingProviders > 0 ? " ok" : " warning"}`} onClick={() => navigateTo("models-cost")}>
              Models {workingProviders}/{providerCount}
            </button>
            <button type="button" className="ide-status-chip" onClick={() => navigateTo("models-cost")}>
              Tokens {formatNumber(totalTokens)}
            </button>
            <button type="button" className={`ide-status-chip${bottomPanelOpen ? " active" : ""}`} onClick={() => setBottomPanelOpen((open) => !open)}>
              Panel
            </button>
            <button type="button" className="ide-title-icon-button" onClick={() => setAboutOpen(true)} title="About RepoDesk" aria-label="About RepoDesk">?</button>
          </div>
        </header>

        <section className={`ide-feedback ${feedback?.tone ?? "neutral"}`} aria-live="polite">
          <div>
            <strong>{activeTabInfo.title}</strong>
            <span>{activeTabInfo.subtitle}</span>
          </div>
          <div className="ide-feedback-action">
            <span>{feedback ? "Last action" : "Ready"}</span>
            <strong>{feedback?.title ?? "Workspace loaded"}</strong>
            <small>{feedback?.detail ?? "Select a Work Item or engineering surface."}</small>
          </div>
        </section>

        <main className="ide-surface-scroll">{renderActiveTab()}</main>

        <WorkbenchBottomPanel
          open={bottomPanelOpen}
          onClose={() => setBottomPanelOpen(false)}
        />
      </section>

      {inspectorOpen ? (
        <WorkspaceInspector
          activeTab={activeTab}
          projectName={projectName ?? ""}
          taskTitle={taskTitle ?? ""}
          hasTask={hasTask}
          dirty={dirty}
          dirtyCount={dirtyCount}
          workingProviders={workingProviders}
          providerCount={providerCount}
          totalTokens={totalTokens}
          onNavigate={navigateTo}
        />
      ) : null}

      <AboutModal
        isOpen={aboutOpen}
        onClose={() => setAboutOpen(false)}
        onGetStarted={() =>
          navigateTo(
            hasProject ? "work" : "projects",
            hasProject ? "Open the active Work Item." : "Connect a repository workspace to begin.",
          )
        }
      />
      <ArtifactViewerModal
        isOpen={Boolean(viewingArtifact)}
        kind={viewingArtifact || ""}
        onClose={() => setViewingArtifact(null)}
      />
      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} commands={commands} />
    </div>
  );
}
