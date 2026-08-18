import { lazy, Suspense, useCallback, useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
import { EconomyMode } from "../features/routing/EconomyControl";
import { IDEHealthIndicator } from "../features/health/IDEHealthIndicator";
import { IDEHealthPanelGate } from "../features/health/IDEHealthPanelGate";
import { useGit } from "../features/git/useGit";
import {
  codeWorkspaceQuickOpen,
  requestCodeWorkspaceOpen,
} from "../shared/api/codeWorkspace";
import { flushCodeWorkspaceDrafts } from "../shared/api/codeDraftPersistence";
import { callCommand } from "../shared/api/queries";
import {
  BOTTOM_PANEL_TAB_EVENT,
  type BottomPanelTab,
} from "../shared/api/workbench";
import { useWorkspace } from "../shared/hooks/useWorkspace";
import type { TabId, Theme } from "../shared/types/api";
import { AboutModal } from "../shared/ui/AboutModal";
import { ArtifactViewerModal } from "../shared/ui/ArtifactViewerModal";
import type { Command } from "../shared/ui/CommandPalette";
import { ErrorBoundary } from "../shared/ui/ErrorBoundary";
import { GlobalLoader } from "../shared/ui/GlobalLoader";
import { errorToMessage, StartupSkeleton } from "../shared/ui/SharedComponents";
import { TabErrorBoundary } from "../shared/ui/TabErrorBoundary";
import { useToast } from "../shared/ui/Toast";
import { ActivityRail } from "./ActivityRail";
import { STORAGE_KEYS } from "./constants";
import { readStoredActiveTab, readStoredEconomyMode, readStoredTheme } from "./storage";
import { APP_TABS, PRIMARY_TABS, renderAppTab } from "./tabs";
import { WorkspaceInspector } from "./WorkspaceInspector";
import { WorkspaceSidebar } from "./WorkspaceSidebar";

const ABOUT_SEEN_KEY = "repodesk.about.seen";
const CommandPalette = lazy(() => import("../shared/ui/CommandPalette").then((module) => ({
  default: module.CommandPalette,
})));
const WorkbenchBottomPanel = lazy(() => import("./WorkbenchBottomPanel").then((module) => ({
  default: module.WorkbenchBottomPanel,
})));

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
  const [paletteActivated, setPaletteActivated] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [viewingArtifact, setViewingArtifact] = useState<string | null>(null);
  const [appVersion, setAppVersion] = useState("1.0.0");
  const [feedback, setFeedback] = useState<ShellFeedback | null>(null);

  // Workbench structural surfaces start closed. `sidebarOpen` remains a
  // transitional internal name because the persisted key is legacy-compatible;
  // the product-level surface is the Navigator.
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [bottomPanelOpen, setBottomPanelOpen] = useState(
    () => window.localStorage.getItem(STORAGE_KEYS.bottomPanelOpen) === "1",
  );
  const [bottomPanelActivated, setBottomPanelActivated] = useState(
    () => window.localStorage.getItem(STORAGE_KEYS.bottomPanelOpen) === "1",
  );
  const [requestedBottomPanelTab, setRequestedBottomPanelTab] = useState<BottomPanelTab | null>(null);

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

  const booting = workspaceLoading;
  const activeTabInfo = APP_TABS.find((tab) => tab.id === activeTab) ?? APP_TABS[0];

  const openPalette = useCallback(() => {
    setPaletteActivated(true);
    setPaletteOpen(true);
  }, []);

  const toggleBottomPanel = useCallback(() => {
    setBottomPanelActivated(true);
    setBottomPanelOpen((open) => !open);
  }, []);

  const openBottomPanel = useCallback((tab?: BottomPanelTab) => {
    setBottomPanelActivated(true);
    if (tab) setRequestedBottomPanelTab(tab);
    setBottomPanelOpen(true);
  }, []);

  const { data: projects = [] } = useQuery({
    queryKey: ["project_list_configs"],
    queryFn: () => invoke<ProjectCommandTarget[]>("project_list_configs"),
    retry: false,
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
      // Transitional foundation behavior: explicit route navigation closes the
      // shell Navigator until route-owned Navigator content is migrated.
      setSidebarOpen(false);
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
    const unlisten = listen<{ requestId: number }>("repodesk-safe-quit-requested", async ({ payload }) => {
      const requestId = payload.requestId;
      let acknowledged = false;
      try {
        await invoke("safe_quit_ack", { requestId });
        acknowledged = true;
        await flushCodeWorkspaceDrafts();
        await invoke("safe_quit_complete", { requestId });
      } catch (error) {
        if (!acknowledged) return;
        await invoke("safe_quit_cancel", { requestId }).catch(() => undefined);
        showFeedback(
          "error",
          "Quit cancelled",
          `Recovery drafts could not be saved: ${errorToMessage(error)}`,
        );
      }
    });
    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, [showFeedback]);

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEYS.activeTab, activeTab);
  }, [activeTab]);

  useEffect(() => {
    // Legacy key retained for backward compatibility; this stores Navigator state.
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

  useEffect(() => {
    const onTabRequest = (event: Event) => {
      const tab = (event as CustomEvent<BottomPanelTab>).detail;
      if (tab === "problems" || tab === "tasks" || tab === "output" || tab === "terminal") {
        openBottomPanel(tab);
      }
    };
    window.addEventListener(BOTTOM_PANEL_TAB_EVENT, onTabRequest);
    return () => window.removeEventListener(BOTTOM_PANEL_TAB_EVENT, onTabRequest);
  }, [openBottomPanel]);

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      const mod = event.metaKey || event.ctrlKey;
      if (!mod) return;

      const key = event.key.toLowerCase();
      const modalOpen = document.querySelector('[role="dialog"][aria-modal="true"]') !== null;
      const togglesOpenPalette = paletteOpen && (key === "k" || (key === "p" && event.shiftKey));
      if (modalOpen && !togglesOpenPalette) return;

      if (key === "k" || (key === "p" && event.shiftKey)) {
        event.preventDefault();
        if (paletteOpen) setPaletteOpen(false);
        else openPalette();
        return;
      }
      if (key === "b") {
        event.preventDefault();
        setSidebarOpen((open) => !open);
        return;
      }
      if (key === "j") {
        event.preventDefault();
        toggleBottomPanel();
        return;
      }
      if (/^[1-9]$/.test(event.key)) {
        const index = Number(event.key) - 1;
        if (index < PRIMARY_TABS.length) {
          event.preventDefault();
          setActiveTab(PRIMARY_TABS[index].id);
          setSidebarOpen(false);
        }
      }
    };

    window.addEventListener("keydown", handler);
    const unlisten = listen("open-command-palette", openPalette);

    return () => {
      window.removeEventListener("keydown", handler);
      void unlisten.then((dispose) => dispose());
    };
  }, [openPalette, paletteOpen, toggleBottomPanel]);

  const searchFileCommands = useCallback(
    async (query: string): Promise<Command[]> => {
      if (!hasProject) return [];
      const matches = await codeWorkspaceQuickOpen(query, 50);
      return matches.map((file) => ({
        id: `file:${file.path}`,
        label: `Open file: ${file.name}`,
        hint: file.path,
        group: "Files",
        keywords: [file.path, file.language, file.status],
        priority: file.status === "clean" ? 0 : 20,
        run: () => {
          requestCodeWorkspaceOpen(file.path);
          navigateTo("code", `Open ${file.path}.`);
        },
      }));
    },
    [hasProject, navigateTo],
  );

  const commands = useMemo<Command[]>(() => {
    const currentCommands: Command[] = [];
    if (hasTask) {
      currentCommands.push({
        id: "current:work",
        label: `Open Work Item: ${taskTitle || "active task"}`,
        hint: projectName ? `Project · ${projectName}` : "Active bounded engineering task",
        group: "Current",
        keywords: ["task", "work item", "scope", "phase"],
        priority: 100,
        run: () => navigateTo("work", "Opened the active Work Item."),
      });
    }
    if (dirty) {
      currentCommands.push({
        id: "current:changes",
        label: `Review ${dirtyCount} workspace change${dirtyCount === 1 ? "" : "s"}`,
        hint: "Git delta · governance · findings",
        group: "Current",
        keywords: ["diff", "git", "review", "changed files"],
        priority: 90,
        run: () => navigateTo("changes", "Review the current workspace delta."),
      });
    }

    const tabCommands: Command[] = APP_TABS.map((tab) => {
      const primaryIndex = PRIMARY_TABS.findIndex((candidate) => candidate.id === tab.id);
      return {
        id: `goto:${tab.id}`,
        label: `Go to ${tab.title}`,
        hint: tab.subtitle,
        group: "Navigate",
        keywords: [tab.group, tab.title, tab.subtitle],
        shortcut: primaryIndex >= 0 ? `⌘${primaryIndex + 1}` : undefined,
        priority: tab.id === activeTab ? 10 : 0,
        run: () => navigateTo(tab.id),
      };
    });

    const shellCommands: Command[] = [
      {
        id: "shell:navigator",
        label: "Toggle Navigator",
        hint: "Project and current engineering context",
        group: "View",
        shortcut: "⌘B",
        keywords: ["navigator", "sidebar", "project"],
        run: () => setSidebarOpen((open) => !open),
      },
      {
        id: "shell:inspector",
        label: "Toggle Inspector",
        hint: "Contextual engineering evidence",
        group: "View",
        keywords: ["inspector", "right panel"],
        run: () => setInspectorOpen((open) => !open),
      },
      {
        id: "shell:bottom-panel",
        label: "Toggle bottom panel",
        hint: "Terminal and verification output",
        group: "View",
        shortcut: "⌘J",
        keywords: ["terminal", "panel", "checks"],
        run: toggleBottomPanel,
      },
    ];

    const workActions: Command[] = [
      {
        id: "action:refresh",
        label: "Refresh workspace",
        hint: "Reload cached project and Work Item data",
        group: "Work",
        keywords: ["reload", "refresh", "sync"],
        run: () =>
          runWithFeedback({
            pending: "Refreshing workspace",
            success: "Workspace refreshed",
            task: async () => {
              await queryClient.invalidateQueries();
            },
          }),
      },
      ...(hasTask ? [
        {
          id: "action:build-context",
          label: "Build bounded context",
          hint: "Prepare the canonical Work Item packet",
          group: "Work",
          keywords: ["context", "prepare", "packet", "tokens"],
          priority: 30,
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
          label: "Run configured checks",
          hint: "Execute verification checks for the active Work Item",
          group: "Work",
          keywords: ["verify", "tests", "checks", "validation"],
          priority: 20,
          run: () =>
            runWithFeedback({
              pending: "Running checks",
              success: "Checks finished",
              task: async () => {
                await callCommand("run_desktop_action", { actionId: "checks-run" });
                await queryClient.invalidateQueries();
                openBottomPanel();
              },
            }),
        },
        {
          id: "action:generate-prompts",
          label: "Generate manual handoff prompts",
          hint: "Prepare external-agent prompt artifacts",
          group: "Work",
          keywords: ["prompt", "handoff", "agent"],
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
      ] satisfies Command[] : []),
    ];

    const projectCommands: Command[] = projects.map((project) => ({
      id: `project:${project.name}`,
      label: `Switch project: ${project.name}`,
      hint: project.path,
      group: "Projects",
      keywords: [project.name, project.path],
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

    const appearanceCommands: Command[] = [
      {
        id: "action:theme-dark",
        label: "Theme: Dark",
        group: "Appearance",
        run: () => setTheme("dark"),
      },
      {
        id: "action:theme-light",
        label: "Theme: Light",
        group: "Appearance",
        run: () => setTheme("light"),
      },
      {
        id: "action:theme-system",
        label: "Theme: Auto",
        group: "Appearance",
        run: () => setTheme("system"),
      },
    ];

    return [
      ...currentCommands,
      ...tabCommands,
      ...workActions,
      ...projectCommands,
      ...shellCommands,
      ...appearanceCommands,
    ];
  }, [
    activeTab,
    dirty,
    dirtyCount,
    hasTask,
    navigateTo,
    projectName,
    projects,
    queryClient,
    runWithFeedback,
    taskTitle,
    openBottomPanel,
    toggleBottomPanel,
  ]);

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
        onToggleBottomPanel={toggleBottomPanel}
        onOpenPalette={openPalette}
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
            {feedback ? (
              <span className={`ide-context-feedback ${feedback.tone}`} title={feedback.detail}>
                {feedback.title}
              </span>
            ) : null}
            <button
              type="button"
              className={`ide-status-chip${dirty ? " warning" : " ok"}`}
              onClick={() => navigateTo("changes")}
            >
              {dirty ? `${dirtyCount} changes` : "Git clean"}
            </button>
            <IDEHealthIndicator />
            <button
              type="button"
              className="ide-title-icon-button"
              onClick={() => setAboutOpen(true)}
              title="About RepoDesk"
              aria-label="About RepoDesk"
            >
              ?
            </button>
          </div>
        </header>

        <main className="ide-surface-scroll">{renderActiveTab()}</main>

        {bottomPanelActivated ? (
          <ErrorBoundary
            scope="bottom-panel"
            resetKeys={[bottomPanelActivated, bottomPanelOpen, requestedBottomPanelTab]}
          >
            <Suspense fallback={bottomPanelOpen ? (
              <section className="workbench-bottom-panel" aria-label="Workbench bottom panel">
                <div className="bottom-panel-empty"><strong>Loading bottom panel…</strong></div>
              </section>
            ) : null}>
              <WorkbenchBottomPanel
                open={bottomPanelOpen}
                onClose={() => setBottomPanelOpen(false)}
                requestedTab={requestedBottomPanelTab}
              />
            </Suspense>
          </ErrorBoundary>
        ) : null}
      </section>

      {inspectorOpen ? (
        <WorkspaceInspector
          activeTab={activeTab}
          projectName={projectName ?? ""}
          taskTitle={taskTitle ?? ""}
          hasTask={hasTask}
          dirty={dirty}
          dirtyCount={dirtyCount}
          onNavigate={navigateTo}
          onClose={() => setInspectorOpen(false)}
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
      {paletteActivated ? (
        <ErrorBoundary scope="command-palette" resetKeys={[paletteActivated, paletteOpen]}>
          <Suspense fallback={paletteOpen ? (
            <div className="cmdk-overlay">
              <div className="cmdk-panel cmdk-panel-v2" role="status">Loading commands…</div>
            </div>
          ) : null}>
            <CommandPalette
              open={paletteOpen}
              onClose={() => setPaletteOpen(false)}
              commands={commands}
              searchCommands={searchFileCommands}
            />
          </Suspense>
        </ErrorBoundary>
      ) : null}
      <IDEHealthPanelGate />
    </div>
  );
}
