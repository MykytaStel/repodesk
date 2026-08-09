import { invoke } from "@tauri-apps/api/core";

export const CODE_WORKSPACE_KEY = ["code", "workspace-v0"] as const;
const CODE_OPEN_REQUEST_KEY = "repodesk.code.open-request";

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

export async function codeWorkspaceSnapshot(): Promise<CodeWorkspaceSnapshot> {
  return invoke("code_workspace_snapshot");
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

/** Lightweight hand-off between separately mounted workspace routes. The path
 * is still fully revalidated by Rust when Code opens it. */
export function requestCodeWorkspaceOpen(path: string): void {
  window.sessionStorage.setItem(CODE_OPEN_REQUEST_KEY, path);
}

export function consumeCodeWorkspaceOpenRequest(): string | null {
  const path = window.sessionStorage.getItem(CODE_OPEN_REQUEST_KEY);
  if (path) window.sessionStorage.removeItem(CODE_OPEN_REQUEST_KEY);
  return path;
}
