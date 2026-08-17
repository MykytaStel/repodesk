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
  type CodeWorkspaceFile,
  type CodeWorkspaceMutationResult,
  type CodeWorkspaceOpenRequest,
} from "../../shared/api/codeWorkspace";
import {
  discardCodeWorkspaceDraft,
  flushCodeWorkspaceDraft,
  stageCodeWorkspaceTabDrafts,
  subscribeCodeDraftPersistence,
  workspaceProjectFromDraftTab,
} from "../../shared/api/codeDraftPersistence";
import { requestChangesOpen } from "../../shared/api/changesNavigation";
import { callCommand } from "../../shared/api/queries";
import { groupByFile, runRepopilotReview } from "../../shared/api/repopilot";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import type { TabId } from "../../shared/types/api";
import { DiffViewer } from "../../shared/ui/DiffViewer";
import {
  EmptyState,
  ErrorState,
  EvidenceState,
  LoadingState,
} from "../../shared/ui/primitives";
import { errorToMessage } from "../../shared/utils/helpers";
import { FindingRow } from "./CodeFindings";
import { CodeProjectSearch } from "./CodeProjectSearch";
import { CodeTabStrip } from "./CodeTabStrip";
import { useCodeWorkspaceActions } from "./CodeWorkspaceActions";
import { CodeWorkspaceToolbar } from "./CodeWorkspaceToolbar";
import { CodeWorkspaceTree } from "./CodeWorkspaceTree";
import {
  codeSaveSemantic,
  codeWorkspaceIndexSemantic,
} from "./codeSemantic";
import {
  fileName,
  libraryTabId,
  rememberCodeSession,
  restoreCodeSession,
  toLibraryTab,
  toWorkspaceTab,
  workspaceTabId,
  type EditorTab,
} from "./codeTabs";
import { useIdeDecisionDialog } from "./IdeDecisionDialog";
import { useIdePreferences } from "./idePreferences";
import { RepositoryIntelligenceDrawer } from "./RepositoryIntelligenceDrawer";
import { SemanticCodeEditor } from "./SemanticCodeEditor";
import "./code-workspace.css";
import "./ide-chrome.css";
import "../routing/routing-feature.css";

const MAX_OPEN_TABS = 8;

type EditorView = "edit" | "diff";
type CodeSideMode = "explorer" | "search";

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
      stageCodeWorkspaceTabDrafts(previousProject, tabs);
      rememberCodeSession(previousProject, tabs, activeTabId);
    }

    sessionProjectRef.current = projectName ?? null;
    const cached = projectName ? restoreCodeSession(projectName) : null;
    setTabs(cached?.tabs ?? []);
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
    stageCodeWorkspaceTabDrafts(projectName, tabs);
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
      const project = workspaceProjectFromDraftTab(tab);
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
      const project = workspaceProjectFromDraftTab(savedTab) ?? projectName ?? "unknown";
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
          const project = workspaceProjectFromDraftTab(target);
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
    return <EmptyState scope="surface" message="Connect a project to open the Code workspace." />;
  }
  if (workspace.isLoading) {
    return <LoadingState scope="surface" message="Indexing repository files…" />;
  }
  if (workspace.isError || !workspace.data) {
    return (
      <ErrorState
        scope="surface"
        title="Code workspace unavailable"
        detail={errorToMessage(workspace.error)}
      />
    );
  }

  const indexSemantic = codeWorkspaceIndexSemantic(workspace.data.truncated);
  const dirtySemantic = dirtyCount > 0 ? codeSaveSemantic("dirty") : null;

  return (
    <div className="code-workspace">
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
        <CodeWorkspaceToolbar
          project={workspace.data.project}
          fileCount={workspace.data.files.length}
          indexSemantic={indexSemantic}
          dirtyCount={dirtyCount}
          dirtySemantic={dirtySemantic}
          canShowRepositoryContext={activeTab?.kind === "workspace"}
          repositoryContextOpen={repoIntelOpen}
          reviewPending={review.isPending}
          reviewTotal={review.data?.total ?? null}
          insightsOpen={insightsOpen}
          onToggleRepositoryContext={() => {
            setRepoIntelOpen((open) => !open);
            setInsightsOpen(false);
          }}
          onAnalyze={() => review.mutate()}
          onToggleInsights={() => {
            setInsightsOpen((open) => !open);
            setRepoIntelOpen(false);
          }}
          onReviewChanges={openChanges}
          reviewFile={activeTab?.kind === "workspace"}
        />

        <CodeTabStrip
          tabs={tabs}
          activeTabId={activeTabId}
          onSelect={(tabId) => {
            setActiveTabId(tabId);
            setView("edit");
          }}
          onClose={(tabId) => void closeTab(tabId)}
        />

        {draftError ? (
          <EvidenceState
            label="Draft recovery"
            state="Backup unavailable"
            tone="attention"
            detail={draftError}
            role="status"
          >
            <button type="button" className="tiny-button" onClick={() => setDraftError(null)}>Dismiss</button>
          </EvidenceState>
        ) : null}

        {workspaceError ? (
          <ErrorState
            title="Code workspace action failed"
            detail={workspaceError}
            action={(
              <div className="button-row">
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
            )}
          />
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
              <EmptyState
                message="Repository is ready."
                hint={`Choose a safe text file in Explorer. Editor budget: ${MAX_OPEN_TABS} open files · 512 KiB per file · conflict-safe saves.`}
              />
            </div>
          ) : view === "diff" && activeTab.kind === "workspace" ? (
            <div className="code-diff-stage">
              {diffLoading ? <LoadingState message="Loading diff…" /> : diff ? <DiffViewer diff={diff} /> : (
                <EmptyState message="No Git diff for this file." />
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
              {review.data.error ? <ErrorState title="RepoPilot analysis failed" detail={review.data.error} /> : null}
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