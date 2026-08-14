import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CODE_OPEN_EVENT,
  CODE_WORKSPACE_KEY,
  codeWorkspaceSnapshot,
  consumeCodeWorkspaceOpenRequest,
  loadCodeWorkspaceDraft,
  requestCodeWorkspaceOpen,
  readCodeLibraryDocument,
  readCodeWorkspaceDocument,
  saveCodeWorkspaceDocument,
  type CodeWorkspaceDocument,
  type CodeWorkspaceFile,
  type CodeWorkspaceFileStatus,
  type CodeWorkspaceMutationResult,
  type CodeWorkspaceOpenRequest,
} from "../../shared/api/codeWorkspace";
import {
  discardCodeWorkspaceDraft,
  flushCodeWorkspaceDraft,
  stageCodeWorkspaceDrafts,
  subscribeCodeDraftPersistence,
} from "../../shared/api/codeDraftPersistence";
import { requestChangesOpen } from "../../shared/api/changesNavigation";
import { callCommand } from "../../shared/api/queries";
import { groupByFile, runRepopilotReview } from "../../shared/api/repopilot";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import type { TabId } from "../../shared/types/api";
import { DiffViewer } from "../../shared/ui/DiffViewer";
import { errorToMessage } from "../../shared/utils/helpers";
import { FindingRow } from "./CodeFindings";
import { CodeProjectSearch } from "./CodeProjectSearch";
import { useCodeWorkspaceActions } from "./CodeWorkspaceActions";
import { CodeWorkspaceTree } from "./CodeWorkspaceTree";
import { IdeIcon } from "./IdeIcon";
import { useIdeDecisionDialog } from "./IdeDecisionDialog";
import { useIdePreferences } from "./idePreferences";
import { LibraryTabBadge } from "./LibraryTabBadge";
import { RepositoryIntelligenceDrawer } from "./RepositoryIntelligenceDrawer";
import { SemanticCodeEditor } from "./SemanticCodeEditor";
import "./code-workspace.css";
import "./ide-chrome.css";
import "../routing/routing-feature.css";

const MAX_OPEN_TABS = 8;
const MAX_CACHED_PROJECT_SESSIONS = 2;

type EditorTab = {
  id: string;
  kind: "workspace" | "library";
  path: string;
  libraryHandle: string | null;
  content: string;
  fingerprint: string;
  language: string;
  bytes: number;
  status: CodeWorkspaceFileStatus;
  dirty: boolean;
  recoveredDraft: boolean;
};

type EditorView = "edit" | "diff";
type CodeSideMode = "explorer" | "search";

type CachedCodeSession = {
  tabs: EditorTab[];
  activeTabId: string | null;
  touchedAt: number;
};

const codeSessionCache = new Map<string, CachedCodeSession>();

const STATUS_LABEL: Record<CodeWorkspaceFileStatus, string> = {
  clean: "",
  modified: "M",
  added: "A",
  deleted: "D",
  untracked: "U",
  renamed: "R",
  conflict: "!",
};

function workspaceTabId(project: string, path: string): string {
  return `workspace:${project}:${path}`;
}

function workspaceTabProject(tab: EditorTab): string | null {
  if (tab.kind !== "workspace") return null;
  const prefix = "workspace:";
  const suffix = `:${tab.path}`;
  if (!tab.id.startsWith(prefix) || !tab.id.endsWith(suffix)) return null;
  return tab.id.slice(prefix.length, tab.id.length - suffix.length) || null;
}

function dirtyDraftSnapshots(project: string, tabs: EditorTab[]) {
  return tabs
    .filter((tab) => tab.kind === "workspace" && tab.dirty && tab.id === workspaceTabId(project, tab.path))
    .map((tab) => ({ path: tab.path, content: tab.content, baseFingerprint: tab.fingerprint }));
}

function libraryTabId(handle: string): string {
  return `library:${handle}`;
}

function cloneTabs(tabs: EditorTab[]): EditorTab[] {
  return tabs.map((tab) => ({ ...tab }));
}

function rememberCodeSession(project: string, tabs: EditorTab[], activeTabId: string | null) {
  const workspaceTabs = tabs.filter((tab) => tab.kind === "workspace");
  codeSessionCache.set(project, {
    tabs: cloneTabs(workspaceTabs),
    activeTabId: workspaceTabs.some((tab) => tab.id === activeTabId) ? activeTabId : null,
    touchedAt: Date.now(),
  });

  if (codeSessionCache.size <= MAX_CACHED_PROJECT_SESSIONS) return;
  const removable = [...codeSessionCache.entries()]
    .filter(([name, session]) => name !== project && session.tabs.every((tab) => !tab.dirty))
    .sort((left, right) => left[1].touchedAt - right[1].touchedAt);
  while (codeSessionCache.size > MAX_CACHED_PROJECT_SESSIONS && removable.length > 0) {
    const [name] = removable.shift()!;
    codeSessionCache.delete(name);
  }
}

function toWorkspaceTab(document: CodeWorkspaceDocument, project: string): EditorTab {
  return {
    id: workspaceTabId(project, document.path),
    kind: "workspace",
    path: document.path,
    libraryHandle: null,
    content: document.content,
    fingerprint: document.fingerprint,
    language: document.language,
    bytes: document.bytes,
    status: document.status,
    dirty: false,
    recoveredDraft: false,
  };
}

function toLibraryTab(document: Awaited<ReturnType<typeof readCodeLibraryDocument>>): EditorTab {
  return {
    id: libraryTabId(document.handle),
    kind: "library",
    path: document.display_path,
    libraryHandle: document.handle,
    content: document.content,
    fingerprint: "",
    language: document.language,
    bytes: document.bytes,
    status: "clean",
    dirty: false,
    recoveredDraft: false,
  };
}

function fileName(path: string): string {
  return path.split("/").pop() || path;
}

export function CodeTab({
  setActiveTab,
}: {
  setActiveTab: (tab: TabId, detail?: string) => void;
}) {
  const { hasProject, projectName } = useWorkspace();
  const queryClient = useQueryClient();
  const [tabs, setTabs] = useState<EditorTab[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [openingPath, setOpeningPath] = useState<string | null>(null);
  const [view, setView] = useState<EditorView>("edit");
  const [diff, setDiff] = useState("");
  const [diffLoading, setDiffLoading] = useState(false);
  const [insightsOpen, setInsightsOpen] = useState(false);
  const [repoIntelOpen, setRepoIntelOpen] = useState(false);
  const [sideMode, setSideMode] = useState<CodeSideMode>("explorer");
  const [workspaceError, setWorkspaceError] = useState<string | null>(null);
  const [draftError, setDraftError] = useState<string | null>(null);
  const sessionProjectRef = useRef<string | null>(null);
  const openingRef = useRef(false);

  const workspace = useQuery({
    queryKey: [...CODE_WORKSPACE_KEY, projectName ?? "none"],
    queryFn: codeWorkspaceSnapshot,
    enabled: hasProject,
    staleTime: 15_000,
    refetchOnWindowFocus: true,
  });

  const review = useMutation({ mutationFn: runRepopilotReview });
  const findings = useMemo(() => groupByFile(review.data), [review.data]);
  const findingsByFile = useMemo(() => new Map(findings.map((group) => [group.file, group])), [findings]);
  const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? null;
  const activeWorkspacePath = activeTab?.kind === "workspace" ? activeTab.path : null;
  const activeFindings = activeWorkspacePath ? findingsByFile.get(activeWorkspacePath) : undefined;
  const dirtyCount = tabs.filter((tab) => tab.dirty).length;
  const idePreferences = useIdePreferences();
  const { confirm: confirmEditorDecision, dialog: editorDecisionDialog } = useIdeDecisionDialog();

  useEffect(() => {
    const previousProject = sessionProjectRef.current;
    if (previousProject && previousProject !== projectName) {
      stageCodeWorkspaceDrafts(previousProject, dirtyDraftSnapshots(previousProject, tabs));
      rememberCodeSession(previousProject, tabs, activeTabId);
    }

    sessionProjectRef.current = projectName ?? null;
    const cached = projectName ? codeSessionCache.get(projectName) : null;
    setTabs(cached ? cloneTabs(cached.tabs) : []);
    setActiveTabId(cached?.activeTabId ?? null);
    setWorkspaceError(null);
    setView("edit");
    setInsightsOpen(false);
    setRepoIntelOpen(false);
    setSideMode("explorer");
  // The state snapshot here intentionally belongs to the previous project.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectName]);

  useEffect(() => {
    const project = sessionProjectRef.current;
    if (project) rememberCodeSession(project, tabs, activeTabId);
  }, [activeTabId, tabs]);

  useEffect(() => {
    if (!projectName) return;
    stageCodeWorkspaceDrafts(projectName, dirtyDraftSnapshots(projectName, tabs));
  }, [projectName, tabs]);

  useEffect(() => subscribeCodeDraftPersistence((error) => {
    if (!error) {
      setDraftError(null);
      return;
    }
    if (sessionProjectRef.current && error.projectName !== sessionProjectRef.current) return;
    setDraftError(`Draft recovery backup failed for ${fileName(error.path)}: ${error.message}`);
  }), []);

  useEffect(() => {
    const handler = (event: BeforeUnloadEvent) => {
      if (!tabs.some((tab) => tab.dirty)) return;
      event.preventDefault();
    };
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [tabs]);

  useEffect(() => {
    if (view !== "diff" || !activeWorkspacePath) return;
    let cancelled = false;
    setDiffLoading(true);
    setDiff("");
    const load = async () => {
      try {
        let next = await callCommand<string>("git_file_diff", { path: activeWorkspacePath, cached: false });
        if (!next.trim()) next = await callCommand<string>("git_file_diff", { path: activeWorkspacePath, cached: true });
        if (!cancelled) setDiff(next.trim());
      } catch (error) {
        if (!cancelled) setDiff(errorToMessage(error));
      } finally {
        if (!cancelled) setDiffLoading(false);
      }
    };
    void load();
    return () => {
      cancelled = true;
    };
  }, [activeWorkspacePath, view]);

  const save = useMutation({
    mutationFn: async (tab: EditorTab) => {
      if (tab.kind !== "workspace") throw new Error("Library documents are read-only.");
      const project = workspaceTabProject(tab);
      if (!project) throw new Error("Workspace tab has no stable project identity.");
      await flushCodeWorkspaceDraft(project, tab.path);
      const result = await saveCodeWorkspaceDocument({
        path: tab.path,
        content: tab.content,
        expected_fingerprint: tab.fingerprint,
      });
      try {
        await discardCodeWorkspaceDraft(project, tab.path);
      } catch (error) {
        setDraftError(`Saved ${fileName(tab.path)}, but its recovery draft could not be cleared: ${errorToMessage(error)}`);
      }
      return result;
    },
    onSuccess: (result, savedTab) => {
      const saved = result.document;
      const project = workspaceTabProject(savedTab) ?? projectName ?? "unknown";
      const refreshed = { ...toWorkspaceTab(saved, project), id: savedTab.id };
      setTabs((current) => current.map((tab) => tab.id === savedTab.id ? refreshed : tab));
      setWorkspaceError(null);
      void queryClient.invalidateQueries({ queryKey: CODE_WORKSPACE_KEY });
      void queryClient.invalidateQueries({ queryKey: ["git"] });
      void queryClient.invalidateQueries({ queryKey: ["work"] });
      void queryClient.invalidateQueries({ queryKey: ["repository"] });
    },
    onError: (error) => setWorkspaceError(errorToMessage(error)),
  });

  const openFile = useCallback(async (file: CodeWorkspaceFile, forceReload = false) => {
    if (file.blocked || openingRef.current) return;
    const project = projectName ?? workspace.data?.project;
    if (!project) return;
    const targetId = workspaceTabId(project, file.path);
    const existing = tabs.find((tab) => tab.id === targetId);
    if (existing && !forceReload) {
      setActiveTabId(targetId);
      setView("edit");
      return;
    }
    if (existing?.dirty && forceReload) {
      const discard = await confirmEditorDecision({
        title: "Reload from disk?",
        message: `Discard unsaved changes in ${fileName(file.path)} and reload the current disk version?`,
        confirmLabel: "Discard and reload",
        danger: true,
      });
      if (!discard) return;
      try {
        await discardCodeWorkspaceDraft(project, file.path);
      } catch (error) {
        setDraftError(`Could not discard the recovery draft for ${fileName(file.path)}: ${errorToMessage(error)}`);
        return;
      }
    }

    let evictId: string | null = null;
    if (!existing && tabs.length >= MAX_OPEN_TABS) {
      evictId = tabs.find((tab) => !tab.dirty)?.id ?? null;
      if (!evictId) {
        setWorkspaceError(`Code Workspace keeps at most ${MAX_OPEN_TABS} open files. Save or close one first.`);
        return;
      }
    }

    openingRef.current = true;
    setOpeningPath(file.path);
    setWorkspaceError(null);
    try {
      const document = await readCodeWorkspaceDocument(file.path);
      let openedTab = toWorkspaceTab(document, project);
      try {
        const recovery = await loadCodeWorkspaceDraft({
          path: document.path,
          current_fingerprint: document.fingerprint,
        }, project);
        if (recovery) {
          const restore = recovery.state === "safe" || await confirmEditorDecision({
            title: "Recovered draft conflicts with disk",
            message: `RepoDesk found an unsaved draft for ${fileName(document.path)}, but the file changed on disk after that draft was created. Restore the draft into the editor without saving it, or keep the current disk version.`,
            confirmLabel: "Restore draft",
            cancelLabel: "Keep disk version",
          });
          if (restore) {
            openedTab = {
              ...openedTab,
              content: recovery.draft.content,
              bytes: new TextEncoder().encode(recovery.draft.content).length,
              dirty: true,
              recoveredDraft: true,
            };
          } else {
            await discardCodeWorkspaceDraft(project, document.path);
          }
        }
      } catch (error) {
        setDraftError(`Draft recovery is unavailable for ${fileName(document.path)}: ${errorToMessage(error)}`);
      }
      setTabs((current) => {
        const withoutEvicted = evictId ? current.filter((tab) => tab.id !== evictId) : current;
        const existingIndex = withoutEvicted.findIndex((tab) => tab.id === openedTab.id);
        if (existingIndex >= 0) {
          const next = [...withoutEvicted];
          next[existingIndex] = openedTab;
          return next;
        }
        return [...withoutEvicted, openedTab];
      });
      setActiveTabId(openedTab.id);
      setView("edit");
    } catch (error) {
      setWorkspaceError(errorToMessage(error));
    } finally {
      openingRef.current = false;
      setOpeningPath(null);
    }
  }, [confirmEditorDecision, projectName, tabs, workspace.data?.project]);

  const openLibrary = useCallback(async (request: CodeWorkspaceOpenRequest) => {
    if (!request.libraryHandle || openingRef.current) return;
    const targetId = libraryTabId(request.libraryHandle);
    const existing = tabs.find((tab) => tab.id === targetId);
    if (existing) {
      setActiveTabId(existing.id);
      setView("edit");
      return;
    }

    let evictId: string | null = null;
    if (tabs.length >= MAX_OPEN_TABS) {
      evictId = tabs.find((tab) => !tab.dirty)?.id ?? null;
      if (!evictId) {
        setWorkspaceError(`Code Workspace keeps at most ${MAX_OPEN_TABS} open files. Save or close one first.`);
        return;
      }
    }

    openingRef.current = true;
    setOpeningPath(request.path);
    setWorkspaceError(null);
    try {
      const document = await readCodeLibraryDocument(request.libraryHandle);
      const tab = toLibraryTab(document);
      setTabs((current) => [
        ...(evictId ? current.filter((item) => item.id !== evictId) : current),
        tab,
      ]);
      setActiveTabId(tab.id);
      setView("edit");
    } catch (error) {
      setWorkspaceError(errorToMessage(error));
    } finally {
      openingRef.current = false;
      setOpeningPath(null);
    }
  }, [tabs]);

  useEffect(() => {
    if (!workspace.data) return;
    const consumeRequest = () => {
      const requested = consumeCodeWorkspaceOpenRequest();
      if (!requested) return;
      if (requested.libraryHandle) {
        void openLibrary(requested);
        return;
      }
      const file = workspace.data.files.find((item) => item.path === requested.path);
      if (!file) {
        setWorkspaceError(`Requested file is not available in the active repository: ${requested.path}`);
        return;
      }
      void openFile(file);
    };

    consumeRequest();
    window.addEventListener(CODE_OPEN_EVENT, consumeRequest);
    return () => window.removeEventListener(CODE_OPEN_EVENT, consumeRequest);
  }, [openFile, openLibrary, workspace.data]);

  const closeTab = async (tabId: string) => {
    const target = tabs.find((tab) => tab.id === tabId);
    if (!target) return;
    if (target.dirty) {
      const discard = await confirmEditorDecision({
        title: "Close unsaved file?",
        message: `Discard unsaved changes in ${fileName(target.path)} and close this editor tab?`,
        confirmLabel: "Discard and close",
        danger: true,
      });
      if (!discard) return;
      if (target.kind === "workspace") {
        try {
          const project = workspaceTabProject(target);
          if (!project) throw new Error("Workspace tab has no stable project identity.");
          await discardCodeWorkspaceDraft(project, target.path);
        } catch (error) {
          setDraftError(`Could not discard the recovery draft for ${fileName(target.path)}: ${errorToMessage(error)}`);
          return;
        }
      }
    }
    const index = tabs.findIndex((tab) => tab.id === tabId);
    const nextTabs = tabs.filter((tab) => tab.id !== tabId);
    setTabs(nextTabs);
    if (activeTabId === tabId) {
      setActiveTabId(nextTabs[Math.min(index, nextTabs.length - 1)]?.id ?? null);
      setView("edit");
    }
  };

  const updateActiveContent = (content: string) => {
    if (!activeTabId) return;
    setTabs((current) => current.map((tab) => (
      tab.id === activeTabId && tab.kind === "workspace" ? { ...tab, content, dirty: true } : tab
    )));
  };

  const openChanges = () => {
    if (activeTab?.kind === "workspace") {
      requestChangesOpen(activeTab.path);
      setActiveTab("changes", `Review the Git delta for ${activeTab.path}.`);
      return;
    }
    setActiveTab("changes", "Review the current Git delta and evidence.");
  };

  const refreshMutationProjections = useCallback(async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: CODE_WORKSPACE_KEY }),
      queryClient.invalidateQueries({ queryKey: ["git"] }),
      queryClient.invalidateQueries({ queryKey: ["work"] }),
      queryClient.invalidateQueries({ queryKey: ["repository"] }),
    ]);
  }, [queryClient]);

  const handleWorkspaceMutation = useCallback(async (result: CodeWorkspaceMutationResult) => {
    setWorkspaceError(null);
    const project = projectName ?? workspace.data?.project;
    if (!project) {
      setWorkspaceError("The active project changed while the workspace mutation was completing.");
      await refreshMutationProjections();
      return;
    }

    if (result.kind === "file_created") {
      const document = await readCodeWorkspaceDocument(result.path);
      const openedTab = toWorkspaceTab(document, project);
      let evictId: string | null = null;
      if (tabs.length >= MAX_OPEN_TABS) {
        evictId = tabs.find((tab) => !tab.dirty)?.id ?? null;
        if (!evictId) {
          setWorkspaceError(`Created ${result.path}, but all ${MAX_OPEN_TABS} editor tabs have unsaved changes. Close or save one to open it.`);
          await refreshMutationProjections();
          return;
        }
      }
      setTabs((current) => [
        ...(evictId ? current.filter((tab) => tab.id !== evictId) : current),
        openedTab,
      ]);
      setActiveTabId(openedTab.id);
      setView("edit");
    } else if (result.kind === "file_renamed" && result.previous_path) {
      const document = await readCodeWorkspaceDocument(result.path);
      const previousId = workspaceTabId(project, result.previous_path);
      const renamedTab = toWorkspaceTab(document, project);
      setTabs((current) => current.map((tab) => (
        tab.id === previousId ? renamedTab : tab
      )));
      setActiveTabId((current) => current === previousId ? renamedTab.id : current);
      setView("edit");
    } else if (result.kind === "file_deleted") {
      const deletedId = workspaceTabId(project, result.path);
      setTabs((current) => current.filter((tab) => tab.id !== deletedId));
      setActiveTabId((current) => current === deletedId ? null : current);
      setView("edit");
    }

    await refreshMutationProjections();
  }, [projectName, refreshMutationProjections, tabs, workspace.data?.project]);

  const workspaceActions = useCodeWorkspaceActions({
    getOpenDocument: (path) => {
      const tab = tabs.find((candidate) => candidate.kind === "workspace" && candidate.path === path);
      return tab ? { path: tab.path, fingerprint: tab.fingerprint, dirty: tab.dirty } : null;
    },
    onMutation: handleWorkspaceMutation,
    onError: setWorkspaceError,
    preferences: idePreferences,
  });

  if (!hasProject) {
    return <div className="focus-empty">Connect a project to open the Code workspace.</div>;
  }
  if (workspace.isLoading) {
    return <div className="focus-empty">Indexing repository files…</div>;
  }
  if (workspace.isError || !workspace.data) {
    return <div className="notice danger">{errorToMessage(workspace.error)}</div>;
  }

  return (
    <div className="code-workspace-v0">
      {sideMode === "search" ? (
        <CodeProjectSearch
          onClose={() => setSideMode("explorer")}
          onOpen={(match) => {
            requestCodeWorkspaceOpen(match.path, {
              line: match.line,
              column: match.column,
              endLine: match.line,
              endColumn: match.end_column,
            });
            setSideMode("explorer");
          }}
        />
      ) : (
        <CodeWorkspaceTree
          files={workspace.data.files}
          activePath={activeWorkspacePath}
          onOpen={(file) => void openFile(file)}
          onSearchProject={() => setSideMode("search")}
          onNewFile={workspaceActions.requestCreateFile}
          onNewFolder={workspaceActions.requestCreateDirectory}
          onRename={workspaceActions.requestRename}
          onDelete={workspaceActions.requestDelete}
          onRefresh={() => void workspace.refetch()}
          isPathDirty={(path) => tabs.some((tab) => tab.kind === "workspace" && tab.path === path && tab.dirty)}
        />
      )}

      {workspaceActions.dialog}
      {editorDecisionDialog}

      <section className="code-editor-workbench">
        <header className="code-workspace-toolbar">
          <div className="code-workspace-title">
            <strong>Code</strong>
            <span>{workspace.data.project}</span>
            <span>{workspace.data.files.length.toLocaleString()} files</span>
            {workspace.data.truncated ? <span className="warn">index capped</span> : null}
            {dirtyCount > 0 ? <span className="warn">{dirtyCount} unsaved</span> : null}
          </div>
          <div className="code-workspace-actions ide-icon-toolbar" role="toolbar" aria-label="Code workspace actions">
            {activeTab?.kind === "workspace" ? (
              <button
                type="button"
                className={`ide-icon-button${repoIntelOpen ? " active" : ""}`}
                aria-label="Repository context"
                title="Repository context"
                onClick={() => {
                  setRepoIntelOpen((open) => !open);
                  setInsightsOpen(false);
                }}
              >
                <IdeIcon name="context" />
              </button>
            ) : null}
            <button
              type="button"
              className="ide-icon-button"
              aria-label={review.isPending ? "Analyzing changes" : "Analyze changes"}
              title={review.isPending ? "Analyzing changes…" : "Analyze changes"}
              disabled={review.isPending}
              onClick={() => review.mutate()}
            >
              <IdeIcon name="analyze" />
            </button>
            {review.data ? (
              <button
                type="button"
                className={`ide-icon-button${insightsOpen ? " active" : ""}`}
                aria-label={`Findings ${review.data.total}`}
                title={`${review.data.total} engineering findings`}
                onClick={() => {
                  setInsightsOpen((open) => !open);
                  setRepoIntelOpen(false);
                }}
              >
                <IdeIcon name="more" />
                <span className="ide-icon-count">{review.data.total > 99 ? "99+" : review.data.total}</span>
              </button>
            ) : null}
            <button
              type="button"
              className="ide-icon-button"
              aria-label={activeTab?.kind === "workspace" ? "Review file change" : "Review changes"}
              title={activeTab?.kind === "workspace" ? "Review file change" : "Review changes"}
              onClick={openChanges}
            >
              <IdeIcon name="changes" />
            </button>
          </div>
        </header>

        <div className="code-tab-strip" role="tablist" aria-label="Open files">
          {tabs.length === 0 ? <span className="code-tabs-empty">Open a file from Explorer.</span> : null}
          {tabs.map((tab) => (
            <div className={`code-file-tab${tab.id === activeTabId ? " active" : ""}`} key={tab.id}>
              <button
                type="button"
                role="tab"
                aria-selected={tab.id === activeTabId}
                className="code-file-tab-select"
                onClick={() => {
                  setActiveTabId(tab.id);
                  setView("edit");
                }}
                title={tab.path}
              >
                <span>{fileName(tab.path)}</span>
                {tab.kind === "library" ? <LibraryTabBadge /> : null}
                {tab.recoveredDraft ? <small className="code-draft-badge">recovered</small> : null}
                {STATUS_LABEL[tab.status] ? <small>{STATUS_LABEL[tab.status]}</small> : null}
                {tab.dirty ? <i aria-label="Unsaved">●</i> : null}
              </button>
              <button
                type="button"
                className="code-tab-close"
                aria-label={`Close ${fileName(tab.path)}`}
                onClick={() => void closeTab(tab.id)}
              >×</button>
            </div>
          ))}
        </div>

        {draftError ? (
          <div className="code-workspace-message warn">
            <span>{draftError}</span>
            <button type="button" className="tiny-button" onClick={() => setDraftError(null)}>Dismiss</button>
          </div>
        ) : null}

        {workspaceError ? (
          <div className="code-workspace-message danger">
            <span>{workspaceError}</span>
            {activeTab?.kind === "workspace" && workspaceError.includes("changed outside RepoDesk") ? (
              <button
                type="button"
                className="tiny-button"
                onClick={() => {
                  const file = workspace.data.files.find((item) => item.path === activeTab.path);
                  if (file) void openFile(file, true);
                }}
              >Reload from disk</button>
            ) : null}
            <button type="button" className="tiny-button" onClick={() => setWorkspaceError(null)}>Dismiss</button>
          </div>
        ) : null}

        <div className="code-document-toolbar">
          <div className="code-document-location">
            {openingPath ? <span>Opening {openingPath}…</span> : activeTab ? (
              <>
                <code>{activeTab.path}</code>
                {activeTab.kind === "library" ? <span className="code-read-only-label">Read only</span> : null}
                {activeTab.recoveredDraft ? <span className="code-recovered-label">Recovered draft · not saved</span> : null}
              </>
            ) : <span>No file open</span>}
          </div>
          {activeTab?.kind === "workspace" ? (
            <div className="code-document-actions">
              <div className="code-view-switch" role="group" aria-label="Code view">
                <button type="button" className={view === "edit" ? "active" : ""} onClick={() => setView("edit")}>Edit</button>
                <button type="button" className={view === "diff" ? "active" : ""} onClick={() => setView("diff")}>Diff</button>
              </div>
              <button
                type="button"
                className="primary-button compact"
                disabled={!activeTab.dirty || save.isPending}
                onClick={() => save.mutate(activeTab)}
              >
                {save.isPending ? "Saving…" : "Save"}
              </button>
            </div>
          ) : null}
        </div>

        <div className="code-document-stage">
          {!activeTab ? (
            <div className="code-editor-empty">
              <strong>Repository is ready.</strong>
              <span>Choose a safe text file in Explorer. Code no longer depends on the Git changed-file list.</span>
              <small>Editor budget: {MAX_OPEN_TABS} open files · 512 KiB per file · conflict-safe saves.</small>
            </div>
          ) : view === "diff" && activeTab.kind === "workspace" ? (
            <div className="code-diff-stage">
              {diffLoading ? <div className="focus-empty compact">Loading diff…</div> : diff ? <DiffViewer diff={diff} /> : (
                <div className="focus-empty compact">No Git diff for this file.</div>
              )}
            </div>
          ) : (
            <SemanticCodeEditor
              path={activeTab.path}
              value={activeTab.content}
              dirty={activeTab.dirty}
              language={activeTab.language}
              bytes={activeTab.bytes}
              status={activeTab.status}
              saving={save.isPending}
              readOnly={activeTab.kind === "library"}
              onChange={updateActiveContent}
              onSave={() => {
                if (activeTab.kind === "workspace") save.mutate(activeTab);
              }}
            />
          )}

          {repoIntelOpen && activeWorkspacePath ? (
            <RepositoryIntelligenceDrawer
              projectName={projectName}
              path={activeWorkspacePath}
              onClose={() => setRepoIntelOpen(false)}
            />
          ) : null}

          {insightsOpen && review.data ? (
            <aside className="code-insights-drawer" aria-label="RepoPilot findings">
              <div className="code-insights-head">
                <div>
                  <strong>Engineering findings</strong>
                  <span>{activeWorkspacePath ? fileName(activeWorkspacePath) : "Current changes"}</span>
                </div>
                <button type="button" onClick={() => setInsightsOpen(false)} aria-label="Close findings">×</button>
              </div>
              {review.data.error ? <div className="notice danger">{review.data.error}</div> : null}
              {activeWorkspacePath ? (
                activeFindings ? (
                  <ul className="findings-list">
                    {activeFindings.findings.map((finding, index) => <FindingRow key={index} finding={finding} />)}
                  </ul>
                ) : <p className="muted">No RepoPilot findings for the active file.</p>
              ) : findings.length === 0 ? (
                <p className="muted">No findings in the current changes.</p>
              ) : (
                findings.slice(0, 8).map((group) => (
                  <div className="code-insight-group" key={group.file}>
                    <strong>{group.file}</strong>
                    <span>{group.total} findings</span>
                  </div>
                ))
              )}
            </aside>
          ) : null}
        </div>
      </section>
    </div>
  );
}
