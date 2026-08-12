import { invoke } from "@tauri-apps/api/core";

export const CODE_WORKSPACE_KEY = ["code", "workspace-v0"] as const;
export const CODE_OPEN_EVENT = "repodesk:open-code";
const CODE_OPEN_REQUEST_KEY = "repodesk.code.open-request";
const CODE_LIBRARY_HANDLE_REQUEST_KEY = "repodesk.code.library-handle-request";
const CODE_LOCATION_REQUEST_KEY = "repodesk.code.location-request";

export type CodeWorkspaceSource = "git_index" | "filesystem_fallback";
export type CodeWorkspaceFileStatus =
  | "clean"
  | "modified"
  | "added"
  | "deleted"
  | "untracked"
  | "renamed"
  | "conflict";

export type CodeWorkspaceFile = {
  path: string;
  name: string;
  extension: string | null;
  language: string;
  bytes: number;
  status: CodeWorkspaceFileStatus;
  blocked: boolean;
};

export type CodeWorkspaceSnapshot = {
  project: string;
  source: CodeWorkspaceSource;
  files: CodeWorkspaceFile[];
  truncated: boolean;
};

export type CodeWorkspaceDocument = {
  path: string;
  content: string;
  bytes: number;
  line_count: number;
  language: string;
  status: CodeWorkspaceFileStatus;
  fingerprint: string;
};

export type CodeWorkspaceSaveResult = {
  document: CodeWorkspaceDocument;
  previous_fingerprint: string;
  changed: boolean;
};

export type CodeWorkspaceMutationResult = {
  path: string;
  previous_path: string | null;
  kind: string;
  language: string | null;
};

export type CodeQuickOpenResult = {
  path: string;
  name: string;
  language: string;
  status: CodeWorkspaceFileStatus;
};

export type CodeLibraryDocument = {
  handle: string;
  display_path: string;
  content: string;
  bytes: number;
  line_count: number;
  language: string;
  read_only: true;
};

export type CodeWorkspaceOpenRequest = {
  path: string;
  libraryHandle: string | null;
};

export type CodeWorkspaceLocation = {
  path: string;
  line: number;
  column: number;
  endLine?: number;
  endColumn?: number;
};

export async function codeWorkspaceSnapshot(): Promise<CodeWorkspaceSnapshot> {
  return invoke("code_workspace_snapshot");
}

export async function codeWorkspaceQuickOpen(query: string, limit = 50): Promise<CodeQuickOpenResult[]> {
  return invoke("code_workspace_quick_open", { query, limit });
}

export async function readCodeWorkspaceDocument(path: string): Promise<CodeWorkspaceDocument> {
  return invoke("code_workspace_read", { relativePath: path });
}

export async function saveCodeWorkspaceDocument(input: {
  path: string;
  content: string;
  expected_fingerprint: string;
}): Promise<CodeWorkspaceSaveResult> {
  return invoke("code_workspace_save", { input });
}

export async function createCodeWorkspaceFile(input: {
  path: string;
  content?: string;
}): Promise<CodeWorkspaceMutationResult> {
  return invoke("code_workspace_create_file", {
    input: { path: input.path, content: input.content ?? "" },
  });
}

export async function createCodeWorkspaceDirectory(path: string): Promise<CodeWorkspaceMutationResult> {
  return invoke("code_workspace_create_directory", { relativePath: path });
}

export async function renameCodeWorkspacePath(input: {
  path: string;
  new_path: string;
  expected_fingerprint?: string | null;
}): Promise<CodeWorkspaceMutationResult> {
  return invoke("code_workspace_rename", { input });
}

export async function deleteCodeWorkspacePath(input: {
  path: string;
  expected_fingerprint?: string | null;
}): Promise<CodeWorkspaceMutationResult> {
  return invoke("code_workspace_delete", { input });
}

export async function readCodeLibraryDocument(handle: string): Promise<CodeLibraryDocument> {
  return invoke("code_library_read", { handle });
}

/**
 * Lightweight hand-off between separately mounted workspace routes. Only the
 * repository-relative path and optional cursor location are persisted for the
 * one-shot transition. Rust still fully revalidates the path before reading it.
 */
export function requestCodeWorkspaceOpen(
  path: string,
  location?: {
    line?: number | null;
    column?: number | null;
    endLine?: number | null;
    endColumn?: number | null;
    libraryHandle?: string | null;
  },
): void {
  window.sessionStorage.setItem(CODE_OPEN_REQUEST_KEY, path);
  if (location?.libraryHandle) {
    window.sessionStorage.setItem(CODE_LIBRARY_HANDLE_REQUEST_KEY, location.libraryHandle);
  } else {
    window.sessionStorage.removeItem(CODE_LIBRARY_HANDLE_REQUEST_KEY);
  }
  if (location?.line && location.line > 0) {
    const request: CodeWorkspaceLocation = {
      path,
      line: Math.max(1, Math.floor(location.line)),
      column: Math.max(1, Math.floor(location.column ?? 1)),
    };
    if (location.endLine && location.endLine > 0) {
      request.endLine = Math.max(request.line, Math.floor(location.endLine));
      request.endColumn = Math.max(1, Math.floor(location.endColumn ?? request.column));
    }
    window.sessionStorage.setItem(CODE_LOCATION_REQUEST_KEY, JSON.stringify(request));
  } else {
    window.sessionStorage.removeItem(CODE_LOCATION_REQUEST_KEY);
  }
  window.dispatchEvent(new CustomEvent(CODE_OPEN_EVENT, { detail: { path } }));
}

export function consumeCodeWorkspaceOpenRequest(): CodeWorkspaceOpenRequest | null {
  const path = window.sessionStorage.getItem(CODE_OPEN_REQUEST_KEY);
  if (path) window.sessionStorage.removeItem(CODE_OPEN_REQUEST_KEY);
  if (!path) return null;
  const libraryHandle = window.sessionStorage.getItem(CODE_LIBRARY_HANDLE_REQUEST_KEY);
  window.sessionStorage.removeItem(CODE_LIBRARY_HANDLE_REQUEST_KEY);
  return { path, libraryHandle };
}

export function consumeCodeWorkspaceLocation(path: string): CodeWorkspaceLocation | null {
  const raw = window.sessionStorage.getItem(CODE_LOCATION_REQUEST_KEY);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as Partial<CodeWorkspaceLocation>;
    if (parsed.path !== path || typeof parsed.line !== "number") return null;
    window.sessionStorage.removeItem(CODE_LOCATION_REQUEST_KEY);
    const location: CodeWorkspaceLocation = {
      path,
      line: Math.max(1, Math.floor(parsed.line)),
      column: Math.max(1, Math.floor(typeof parsed.column === "number" ? parsed.column : 1)),
    };
    if (typeof parsed.endLine === "number" && parsed.endLine > 0) {
      location.endLine = Math.max(location.line, Math.floor(parsed.endLine));
      location.endColumn = Math.max(1, Math.floor(
        typeof parsed.endColumn === "number" ? parsed.endColumn : location.column,
      ));
    }
    return location;
  } catch {
    window.sessionStorage.removeItem(CODE_LOCATION_REQUEST_KEY);
    return null;
  }
}
