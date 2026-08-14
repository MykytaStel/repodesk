from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one replacement target, found {count}")
    file.write_text(text.replace(old, new, 1))


# Project-bind every draft IPC call. Legacy callers may still omit projectName.
replace_once(
    "apps/desktop/src/shared/api/codeWorkspace.ts",
    '''export async function saveCodeWorkspaceDraft(input: {
  path: string;
  content: string;
  base_fingerprint: string;
}): Promise<CodeDraftRecord> {
  return invoke("code_workspace_draft_save", { input });
}

export async function loadCodeWorkspaceDraft(input: {
  path: string;
  current_fingerprint: string;
}): Promise<CodeDraftRecovery | null> {
  return invoke("code_workspace_draft_load", { input });
}

export async function deleteCodeWorkspaceDraft(path: string): Promise<boolean> {
  return invoke("code_workspace_draft_delete", { relativePath: path });
}
''',
    '''export async function saveCodeWorkspaceDraft(input: {
  path: string;
  content: string;
  base_fingerprint: string;
}, projectName?: string | null): Promise<CodeDraftRecord> {
  return invoke("code_workspace_draft_save", { input, projectName: projectName ?? null });
}

export async function loadCodeWorkspaceDraft(input: {
  path: string;
  current_fingerprint: string;
}, projectName?: string | null): Promise<CodeDraftRecovery | null> {
  return invoke("code_workspace_draft_load", { input, projectName: projectName ?? null });
}

export async function deleteCodeWorkspaceDraft(path: string, projectName?: string | null): Promise<boolean> {
  return invoke("code_workspace_draft_delete", { relativePath: path, projectName: projectName ?? null });
}
''',
)

# Native tray Quit becomes a two-phase shell handshake.
replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    "mod code_workspace;\npub mod commands;",
    "mod code_workspace;\npub mod commands;\nmod quit;",
)
replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    ".manage(terminal::TerminalManager::default())",
    ".manage(terminal::TerminalManager::default())\n        .manage(quit::QuitCoordinator::default())",
)
replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    '''                    if event.id.as_ref() == "quit" {
                        app.exit(0);
                    }
''',
    '''                    if event.id.as_ref() == "quit" {
                        quit::request_safe_quit(app);
                    }
''',
)
replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    '''        .invoke_handler(tauri::generate_handler![
            terminal::terminal_create,
''',
    '''        .invoke_handler(tauri::generate_handler![
            quit::safe_quit_ack,
            quit::safe_quit_complete,
            quit::safe_quit_cancel,
            terminal::terminal_create,
''',
)

# The always-mounted shell owns the quit flush, not the lazy Code route.
replace_once(
    "apps/desktop/src/app/App.tsx",
    '''} from "../shared/api/codeWorkspace";
import { callCommand } from "../shared/api/queries";
''',
    '''} from "../shared/api/codeWorkspace";
import { flushCodeWorkspaceDrafts } from "../shared/api/codeDraftPersistence";
import { callCommand } from "../shared/api/queries";
''',
)
replace_once(
    "apps/desktop/src/app/App.tsx",
    '''  }, []);

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEYS.activeTab, activeTab);
''',
    '''  }, []);

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
''',
)

# Move debounce/write ownership out of the large editor route.
replace_once(
    "apps/desktop/src/features/code/CodeTab.tsx",
    '''  consumeCodeWorkspaceOpenRequest,
  deleteCodeWorkspaceDraft,
  loadCodeWorkspaceDraft,
''',
    '''  consumeCodeWorkspaceOpenRequest,
  loadCodeWorkspaceDraft,
''',
)
replace_once(
    "apps/desktop/src/features/code/CodeTab.tsx",
    '''  readCodeWorkspaceDocument,
  saveCodeWorkspaceDocument,
  saveCodeWorkspaceDraft,
''',
    '''  readCodeWorkspaceDocument,
  saveCodeWorkspaceDocument,
''',
)
replace_once(
    "apps/desktop/src/features/code/CodeTab.tsx",
    '''} from "../../shared/api/codeWorkspace";
import { requestChangesOpen } from "../../shared/api/changesNavigation";
''',
    '''} from "../../shared/api/codeWorkspace";
import {
  discardCodeWorkspaceDraft,
  flushCodeWorkspaceDraft,
  stageCodeWorkspaceDrafts,
  subscribeCodeDraftPersistence,
} from "../../shared/api/codeDraftPersistence";
import { requestChangesOpen } from "../../shared/api/changesNavigation";
''',
)
replace_once(
    "apps/desktop/src/features/code/CodeTab.tsx",
    '''function workspaceTabId(project: string, path: string): string {
  return `workspace:${project}:${path}`;
}

function libraryTabId(handle: string): string {
''',
    '''function workspaceTabId(project: string, path: string): string {
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
''',
)
replace_once(
    "apps/desktop/src/features/code/CodeTab.tsx",
    '''  const sessionProjectRef = useRef<string | null>(null);
  const openingRef = useRef(false);
  const draftWritesRef = useRef(new Map<string, Promise<void>>());
''',
    '''  const sessionProjectRef = useRef<string | null>(null);
  const openingRef = useRef(false);
''',
)
replace_once(
    "apps/desktop/src/features/code/CodeTab.tsx",
    '''    if (previousProject && previousProject !== projectName) {
      rememberCodeSession(previousProject, tabs, activeTabId);
    }
''',
    '''    if (previousProject && previousProject !== projectName) {
      stageCodeWorkspaceDrafts(previousProject, dirtyDraftSnapshots(previousProject, tabs));
      rememberCodeSession(previousProject, tabs, activeTabId);
    }
''',
)
replace_once(
    "apps/desktop/src/features/code/CodeTab.tsx",
    '''  useEffect(() => {
    const project = sessionProjectRef.current;
    if (project) rememberCodeSession(project, tabs, activeTabId);
  }, [activeTabId, tabs]);

  useEffect(() => {
    const handler = (event: BeforeUnloadEvent) => {
''',
    '''  useEffect(() => {
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
''',
)
replace_once(
    "apps/desktop/src/features/code/CodeTab.tsx",
    '''  const persistDraft = useCallback((tab: EditorTab): Promise<void> => {
    if (tab.kind !== "workspace" || !tab.dirty) return Promise.resolve();

    const previous = draftWritesRef.current.get(tab.path) ?? Promise.resolve();
    const write = previous.then(() => saveCodeWorkspaceDraft({
      path: tab.path,
      content: tab.content,
      base_fingerprint: tab.fingerprint,
    })).then(() => {
      setDraftError(null);
    }).catch((error) => {
      setDraftError(`Draft recovery backup failed for ${fileName(tab.path)}: ${errorToMessage(error)}`);
    });

    let tracked: Promise<void>;
    tracked = write.finally(() => {
      if (draftWritesRef.current.get(tab.path) === tracked) {
        draftWritesRef.current.delete(tab.path);
      }
    });
    draftWritesRef.current.set(tab.path, tracked);
    return tracked;
  }, []);

  const discardPersistedDraft = useCallback(async (path: string) => {
    const pending = draftWritesRef.current.get(path);
    if (pending) await pending;
    await deleteCodeWorkspaceDraft(path);
  }, []);

  useEffect(() => {
    const dirtyWorkspaceTabs = tabs.filter((tab) => tab.kind === "workspace" && tab.dirty);
    if (dirtyWorkspaceTabs.length === 0) return;

    const timer = window.setTimeout(() => {
      for (const tab of dirtyWorkspaceTabs) void persistDraft(tab);
    }, 600);
    return () => window.clearTimeout(timer);
  }, [persistDraft, tabs]);

''',
    '''''',
)
replace_once(
    "apps/desktop/src/features/code/CodeTab.tsx",
    '''  const save = useMutation({
    mutationFn: async (tab: EditorTab) => {
      if (tab.kind !== "workspace") throw new Error("Library documents are read-only.");
      const pendingDraft = draftWritesRef.current.get(tab.path);
      if (pendingDraft) await pendingDraft;
      const result = await saveCodeWorkspaceDocument({
        path: tab.path,
        content: tab.content,
        expected_fingerprint: tab.fingerprint,
      });
      try {
        await deleteCodeWorkspaceDraft(tab.path);
      } catch (error) {
        setDraftError(`Saved ${fileName(tab.path)}, but its recovery draft could not be cleared: ${errorToMessage(error)}`);
      }
      return result;
    },
    onSuccess: (result, savedTab) => {
      const saved = result.document;
      const project = sessionProjectRef.current ?? projectName ?? "unknown";
''',
    '''  const save = useMutation({
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
''',
)
replace_once(
    "apps/desktop/src/features/code/CodeTab.tsx",
    '''        await discardPersistedDraft(file.path);
''',
    '''        await discardCodeWorkspaceDraft(project, file.path);
''',
)
replace_once(
    "apps/desktop/src/features/code/CodeTab.tsx",
    '''        const recovery = await loadCodeWorkspaceDraft({
          path: document.path,
          current_fingerprint: document.fingerprint,
        });
''',
    '''        const recovery = await loadCodeWorkspaceDraft({
          path: document.path,
          current_fingerprint: document.fingerprint,
        }, project);
''',
)
replace_once(
    "apps/desktop/src/features/code/CodeTab.tsx",
    '''            await discardPersistedDraft(document.path);
''',
    '''            await discardCodeWorkspaceDraft(project, document.path);
''',
)
replace_once(
    "apps/desktop/src/features/code/CodeTab.tsx",
    '''  }, [confirmEditorDecision, discardPersistedDraft, projectName, tabs, workspace.data?.project]);
''',
    '''  }, [confirmEditorDecision, projectName, tabs, workspace.data?.project]);
''',
)
replace_once(
    "apps/desktop/src/features/code/CodeTab.tsx",
    '''      if (target.kind === "workspace") {
        try {
          await discardPersistedDraft(target.path);
        } catch (error) {
''',
    '''      if (target.kind === "workspace") {
        try {
          const project = workspaceTabProject(target);
          if (!project) throw new Error("Workspace tab has no stable project identity.");
          await discardCodeWorkspaceDraft(project, target.path);
        } catch (error) {
''',
)
