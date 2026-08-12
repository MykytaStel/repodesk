export const CHANGES_OPEN_EVENT = "repodesk:open-changes";
const CHANGES_OPEN_REQUEST_KEY = "repodesk.changes.open-request";

/**
 * One-shot frontend handoff used when another workbench wants Changes to focus
 * a repository-relative path. The Changes surface still derives the authoritative
 * changed-file set from Git/engineering evidence; this request only carries UI
 * intent and can never manufacture a changed file.
 */
export function requestChangesOpen(path: string): void {
  const normalized = path.trim();
  if (!normalized) return;
  window.sessionStorage.setItem(CHANGES_OPEN_REQUEST_KEY, normalized);
  window.dispatchEvent(new CustomEvent(CHANGES_OPEN_EVENT, { detail: { path: normalized } }));
}

export function consumeChangesOpenRequest(): string | null {
  const path = window.sessionStorage.getItem(CHANGES_OPEN_REQUEST_KEY);
  if (path) window.sessionStorage.removeItem(CHANGES_OPEN_REQUEST_KEY);
  return path?.trim() || null;
}
