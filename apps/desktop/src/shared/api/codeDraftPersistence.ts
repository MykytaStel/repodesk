import {
  deleteCodeWorkspaceDraft,
  saveCodeWorkspaceDraft,
} from "./codeWorkspace";

const DRAFT_DEBOUNCE_MS = 600;

type DraftSnapshot = {
  projectName: string;
  path: string;
  content: string;
  baseFingerprint: string;
};

type StagedDraft = {
  version: number;
  snapshot: DraftSnapshot;
};

export type CodeDraftPersistenceError = {
  projectName: string;
  path: string;
  message: string;
};

type ErrorListener = (error: CodeDraftPersistenceError | null) => void;

const staged = new Map<string, StagedDraft>();
const writeChains = new Map<string, Promise<void>>();
const listeners = new Set<ErrorListener>();
let version = 0;
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

function draftKey(projectName: string, path: string): string {
  return `${projectName}\0${path}`;
}

function notify(error: CodeDraftPersistenceError | null) {
  for (const listener of listeners) listener(error);
}

function cancelDebounce() {
  if (debounceTimer == null) return;
  clearTimeout(debounceTimer);
  debounceTimer = null;
}

function scheduleDebounce() {
  cancelDebounce();
  if (staged.size === 0) return;
  debounceTimer = setTimeout(() => {
    debounceTimer = null;
    void flushCodeWorkspaceDrafts().catch(() => {
      // The coordinator reports the concrete failure to subscribers. Normal
      // debounce persistence remains best-effort; safe quit awaits and handles
      // the rejected promise explicitly.
    });
  }, DRAFT_DEBOUNCE_MS);
}

function queueWrite(entry: StagedDraft): Promise<void> {
  const { snapshot } = entry;
  const key = draftKey(snapshot.projectName, snapshot.path);
  const previous = writeChains.get(key) ?? Promise.resolve();
  const write = previous
    .catch(() => {
      // A prior failed write must not permanently poison the per-file queue.
    })
    .then(async () => {
      await saveCodeWorkspaceDraft(
        {
          path: snapshot.path,
          content: snapshot.content,
          base_fingerprint: snapshot.baseFingerprint,
        },
        snapshot.projectName,
      );
      const current = staged.get(key);
      if (current?.version === entry.version) staged.delete(key);
      notify(null);
    })
    .catch((error) => {
      const message = error instanceof Error ? error.message : String(error);
      notify({
        projectName: snapshot.projectName,
        path: snapshot.path,
        message,
      });
      throw error;
    });

  let tracked: Promise<void>;
  tracked = write.finally(() => {
    if (writeChains.get(key) === tracked) writeChains.delete(key);
  });
  writeChains.set(key, tracked);
  return tracked;
}

async function flushEntries(entries: StagedDraft[]): Promise<void> {
  await Promise.all(entries.map(queueWrite));
}

export function stageCodeWorkspaceDrafts(
  projectName: string,
  drafts: Array<{ path: string; content: string; baseFingerprint: string }>,
): void {
  for (const draft of drafts) {
    const key = draftKey(projectName, draft.path);
    const current = staged.get(key)?.snapshot;
    if (
      current
      && current.content === draft.content
      && current.baseFingerprint === draft.baseFingerprint
    ) {
      continue;
    }
    version += 1;
    staged.set(key, {
      version,
      snapshot: {
        projectName,
        path: draft.path,
        content: draft.content,
        baseFingerprint: draft.baseFingerprint,
      },
    });
  }
  scheduleDebounce();
}

export function unstageCodeWorkspaceDraft(projectName: string, path: string): void {
  staged.delete(draftKey(projectName, path));
  if (staged.size === 0) cancelDebounce();
}

export async function flushCodeWorkspaceDraft(projectName: string, path: string): Promise<void> {
  cancelDebounce();
  const key = draftKey(projectName, path);
  const current = staged.get(key);
  if (current) await queueWrite(current);
  const pending = writeChains.get(key);
  if (pending) await pending;
  scheduleDebounce();
}

/**
 * Persist every latest dirty snapshot. The loop handles edits that arrive while
 * an earlier batch is being written: a newer version remains staged and is
 * picked up by the next pass. Memory is O(number of dirty open tabs), not O(edits).
 */
export async function flushCodeWorkspaceDrafts(): Promise<void> {
  cancelDebounce();
  while (staged.size > 0) {
    const batch = [...staged.values()];
    await flushEntries(batch);
  }
  const pending = [...writeChains.values()];
  if (pending.length > 0) await Promise.all(pending);
}

export async function discardCodeWorkspaceDraft(projectName: string, path: string): Promise<void> {
  await flushCodeWorkspaceDraft(projectName, path);
  unstageCodeWorkspaceDraft(projectName, path);
  await deleteCodeWorkspaceDraft(path, projectName);
}

export function subscribeCodeDraftPersistence(listener: ErrorListener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function stagedCodeDraftCount(): number {
  return staged.size;
}
