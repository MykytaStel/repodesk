import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CODE_OPEN_EVENT,
  CODE_WORKSPACE_KEY,
  codeWorkspaceSnapshot,
  consumeCodeWorkspaceOpenRequest,
  readCodeLibraryDocument,
  readCodeWorkspaceDocument,
  saveCodeWorkspaceDocument,
  type CodeWorkspaceDocument,
  type CodeWorkspaceFile,
  type CodeWorkspaceFileStatus,
  type CodeWorkspaceOpenRequest,
} from "../../shared/api/codeWorkspace";
import { callCommand } from "../../shared/api/queries";
import { groupByFile, runRepopilotReview } from "../../shared/api/repopilot";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import type { TabId } from "../../shared/types/api";
import { DiffViewer } from "../../shared/ui/DiffViewer";
import { errorToMessage } from "../../shared/utils/helpers";
import { FindingRow } from "./CodeFindings";
import { CodeWorkspaceTree } from "./CodeWorkspaceTree";
import { LibraryTabBadge } from "./LibraryTabBadge";
import { RepositoryIntelligenceDrawer } from "./RepositoryIntelligenceDrawer";
import { SemanticCodeEditor } from "./SemanticCodeEditor";
import "./code-workspace.css";

const MAX_OPEN_TABS = 8;
const MAX_CACHED_PROJECT_SESSIONS = 2;

type EditorTab = {
  kind: "workspace" | "library";
  path: string;
  libraryHandle: string | null;
  content: string;
  fingerprint: string;
  language: string;
  bytes: number;
  status: CodeWorkspaceFileStatus;
  dirty: boolean;
};

type EditorView = "edit" | "diff";

type CachedCodeSession = {
  tabs: EditorTab[];
  activePath: string | null;
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

function cloneTabs(tabs: EditorTab[]): EditorTab[] {
  return tabs.map((tab) => ({ ...tab }));
}

function rememberCodeSession(project: string, tabs: EditorTab[], activePath: string | null) {
  const workspaceTabs = tabs.filter((tab) => tab.kind === "workspace");
  codeSessionCache.set(project, {
    tabs: cloneTabs(workspaceTabs),
    activePath: workspaceTabs.some((tab) => tab.path === activePath) ? activePath : null,
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

function toTab(document: CodeWorkspaceDocument): EditorTab {
  return {
    kind: "workspace",
    path: document.path,
    libraryHandle: null,
    content: document.content,
    fingerprint: document.fingerprint,
    language: document.language,
    bytes: document.bytes,
    status: document.status,
    dirty: false,
  };
}

function toLibraryTab(document: Awaited<ReturnType<typeof readCodeLibraryDocument>>): EditorTab {
  return {
    kind: "library",
    path: document.display_path,
    libraryHandle: document.handle,
    content: document.content,
    fingerprint: "",
    language: document.language,
    bytes: document.bytes,
    status: "clean",
    dirty: false,
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
  const [activePath, setActivePath] = useState<string | null>(null);
  const [openingPath, setOpeningPath] = useState<string | null>(null);
  const [view, setView] = useState<EditorView>("edit");
  const [diff, setDiff] = useState("");
  const [diffLoading, setDiffLoading] = useState(false);
  const [insightsOpen, setInsightsOpen] = useState(false);
  const [repoIntelOpen, setRepoIntelOpen] = useState(false);
  const [workspaceError, setWorkspaceError] = useState<string | null>(null);
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
  const activeTab = tabs.find((tab) => tab.path === activePath) ?? null;
  const activeFindings = activePath ? findingsByFile.get(activePath) : undefined;
  const dirtyCount = tabs.filter((tab) => tab.dirty).length;

  useEffect(() => {
    const previousProject = sessionProjectRef.current;
    if (previousProject && previousProject !== projectName) {
      rememberCodeSession(previousProject, tabs, activePath);
    }

    sessionProjectRef.current = projectName ?? null;
    const cached = projectName ? codeSessionCache.get(projectName) : null;
    setTabs(cached ? cloneTabs(cached.tabs) : []);
    setActivePath(cached?.activePath ?? null);
    setWorkspaceError(null);
    setView("edit");
    setInsightsOpen(false);
    setRepoIntelOpen(false);
  // The state snapshot here intentionally belongs to the previous project.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectName]);

  useEffect(() => {
    const project = sessionProjectRef.current;
    if (project) rememberCodeSession(project, tabs, activePath);
  }, [activePath, tabs]);

  useEffect(() => {
    const handler = (event: BeforeUnloadEvent) => {
      if (!tabs.some((tab) => tab.dirty)) return;
      event.preventDefault();
    };
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [tabs]);

  useEffect(() => {
    if (view !== "diff" || !activePath || activeTab?.kind !== "workspace") return;
    let cancelled = false;
    setDiffLoading(true);
    setDiff("");
    const load = async () => {
      try {
        let next = await callCommand<string>("git_file_diff", { path: activePath, cached: false });
        if (!next.trim()) next = await callCommand<string>("git_file_diff", { path: activePath, cached: true });
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
  }, [activePath, activeTab?.kind, view]);

  const save = useMutation({
    mutationFn: (tab: EditorTab) => saveCodeWorkspaceDocument({
      path: tab.path,
      content: tab.content,
      expected_fingerprint: tab.fingerprint,
    }),
    onSuccess: (result) => {
      const saved = result.document;
      setTabs((current) => current.map((tab) => tab.path === saved.path ? toTab(saved) : tab));
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
    const existing = tabs.find((tab) => tab.path === file.path);
    if (existing && !forceReload) {
      setActivePath(file.path);
      setView("edit");
      return;
    }
    if (existing?.dirty && forceReload && !window.confirm(`Discard unsaved changes in ${fileName(file.path)} and reload from disk?`)) {
      return;
    }

    let evictPath: string | null = null;
    if (!existing && tabs.length >= MAX_OPEN_TABS) {
      evictPath = tabs.find((tab) => !tab.dirty)?.path ?? null;
      if (!evictPath) {
        setWorkspaceError(`Code Workspace keeps at most ${MAX_OPEN_TABS} open files. Save or close one first.`);
        return;
      }
    }

    openingRef.current = true;
    setOpeningPath(file.path);
    setWorkspaceError(null);
    try {
      const document = await readCodeWorkspaceDocument(file.path);
      setTabs((current) => {
        const withoutEvicted = evictPath ? current.filter((tab) => tab.path !== evictPath) : current;
        const existingIndex = withoutEvicted.findIndex((tab) => tab.path === document.path);
        if (existingIndex >= 0) {
          const next = [...withoutEvicted];
          next[existingIndex] = toTab(document);
          return next;
        }
        return [...withoutEvicted, toTab(document)];
      });
      setActivePath(document.path);
      setView("edit");
    } catch (error) {
      setWorkspaceError(errorToMessage(error));
    } finally {
      openingRef.current = false;
      setOpeningPath(null);
    }
  }, [tabs]);

  const openLibrary = useCallback(async (request: CodeWorkspaceOpenRequest) => {
    if (!request.libraryHandle || openingRef.current) return;
    const existing = tabs.find((tab) => tab.kind === "library" && tab.path === request.path);
    if (existing) {
      setActivePath(existing.path);
      setView("edit");
      return;
    }

    let evictPath: string | null = null;
    if (tabs.length >= MAX_OPEN_TABS) {
      evictPath = tabs.find((tab) => !tab.dirty)?.path ?? null;
      if (!evictPath) {
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
        ...(evictPath ? current.filter((item) => item.path !== evictPath) : current),
        tab,
      ]);
      setActivePath(tab.path);
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

  const closeTab = (path: string) => {
    const target = tabs.find((tab) => tab.path === path);
    if (target?.dirty && !window.confirm(`Discard unsaved changes in ${fileName(path)}?`)) return;
    const index = tabs.findIndex((tab) => tab.path === path);
    const nextTabs = tabs.filter((tab) => tab.path !== path);
    setTabs(nextTabs);
    if (activePath === path) {
      setActivePath(nextTabs[Math.min(index, nextTabs.length - 1)]?.path ?? null);
      setView("edit");
    }
  };

  const updateActiveContent = (content: string) => {
    if (!activePath) return;
    setTabs((current) => current.map((tab) => (
      tab.path === activePath && tab.kind === "workspace" ? { ...tab, content, dirty: true } : tab
    )));
  };

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
      <CodeWorkspaceTree files={workspace.data.files} activePath={activePath} onOpen={(file) => void openFile(file)} />

      <section className="code-editor-workbench">
        <header className="code-workspace-toolbar">
          <div className="code-workspace-title">
            <strong>Code</strong>
            <span>{workspace.data.project}</span>
            <span>{workspace.data.files.length.toLocaleString()} files</span>
            {workspace.data.truncated ? <span className="warn">index capped</span> : null}
            {dirtyCount > 0 ? <span className="warn">{dirtyCount} unsaved</span> : null}
          </div>
          <div className="code-workspace-actions">
            {activeTab?.kind === "workspace" ? (
              <button
                type="button"
                className={`tiny-button${repoIntelOpen ? " active" : ""}`}
                onClick={() => {
                  setRepoIntelOpen((open) => !open);
                  setInsightsOpen(false);
                }}
              >Repo context</button>
            ) : null}
            <button
              type="button"
              className="tiny-button"
              disabled={review.isPending}
              onClick={() => review.mutate()}
            >
              {review.isPending ? "Analyzing…" : "Analyze changes"}
            </button>
            {review.data ? (
              <button
                type="button"
                className={`tiny-button${insightsOpen ? " active" : ""}`}
                onClick={() => {
                  setInsightsOpen((open) => !open);
                  setRepoIntelOpen(false);
                }}
              >
                Findings {review.data.total}
              </button>
            ) : null}
            <button type="button" className="tiny-button" onClick={() => setActiveTab("changes", "Review the current Git delta and evidence.")}>Review changes</button>
            <button type="button" className="tiny-button" onClick={() => void workspace.refetch()}>Refresh tree</button>
          </div>
        </header>

        <div className="code-tab-strip" role="tablist" aria-label="Open files">
          {tabs.length === 0 ? <span className="code-tabs-empty">Open a file from Explorer.</span> : null}
          {tabs.map((tab) => (
            <div className={`code-file-tab${tab.path === activePath ? " active" : ""}`} key={tab.path}>
              <button
                type="button"
                role="tab"
                aria-selected={tab.path === activePath}
                className="code-file-tab-select"
                onClick={() => {
                  setActivePath(tab.path);
                  setView("edit");
                }}
                title={tab.path}
              >
                <span>{fileName(tab.path)}</span>
                {tab.kind === "library" ? <LibraryTabBadge /> : null}
                {STATUS_LABEL[tab.status] ? <small>{STATUS_LABEL[tab.status]}</small> : null}
                {tab.dirty ? <i aria-label="Unsaved">●</i> : null}
              </button>
              <button
                type="button"
                className="code-tab-close"
                aria-label={`Close ${fileName(tab.path)}`}
                onClick={() => closeTab(tab.path)}
              >×</button>
            </div>
          ))}
        </div>

        {workspaceError ? (
          <div className="code-workspace-message danger">
            <span>{workspaceError}</span>
            {activeTab && workspaceError.includes("changed outside RepoDesk") ? (
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
              <><code>{activeTab.path}</code>{activeTab.kind === "library" ? <span className="code-read-only-label">Read only</span> : null}</>
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

          {repoIntelOpen && activePath ? (
            <RepositoryIntelligenceDrawer
              projectName={projectName}
              path={activePath}
              onClose={() => setRepoIntelOpen(false)}
            />
          ) : null}

          {insightsOpen && review.data ? (
            <aside className="code-insights-drawer" aria-label="RepoPilot findings">
              <div className="code-insights-head">
                <div>
                  <strong>Engineering findings</strong>
                  <span>{activePath ? fileName(activePath) : "Current changes"}</span>
                </div>
                <button type="button" onClick={() => setInsightsOpen(false)} aria-label="Close findings">×</button>
              </div>
              {review.data.error ? <div className="notice danger">{review.data.error}</div> : null}
              {activePath ? (
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
