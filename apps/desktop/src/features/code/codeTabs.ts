import type {
  CodeLibraryDocument,
  CodeWorkspaceDocument,
  CodeWorkspaceFileStatus,
} from "../../shared/api/codeWorkspace";

const MAX_CACHED_PROJECT_SESSIONS = 2;

export type EditorTab = {
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

type CachedCodeSession = {
  tabs: EditorTab[];
  activeTabId: string | null;
  touchedAt: number;
};

const codeSessionCache = new Map<string, CachedCodeSession>();

export function workspaceTabId(project: string, path: string): string {
  return `workspace:${project}:${path}`;
}

export function libraryTabId(handle: string): string {
  return `library:${handle}`;
}

function cloneTabs(tabs: EditorTab[]): EditorTab[] {
  return tabs.map((tab) => ({ ...tab }));
}

export function rememberCodeSession(project: string, tabs: EditorTab[], activeTabId: string | null) {
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

export function restoreCodeSession(project: string): { tabs: EditorTab[]; activeTabId: string | null } | null {
  const cached = codeSessionCache.get(project);
  if (!cached) return null;
  return {
    tabs: cloneTabs(cached.tabs),
    activeTabId: cached.activeTabId,
  };
}

export function toWorkspaceTab(document: CodeWorkspaceDocument, project: string): EditorTab {
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

export function toLibraryTab(document: CodeLibraryDocument): EditorTab {
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

export function fileName(path: string): string {
  return path.split("/").pop() || path;
}
