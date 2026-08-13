import { useEffect, useRef, useState, type FormEvent } from "react";
import {
  createCodeWorkspaceDirectory,
  createCodeWorkspaceFile,
  deleteCodeWorkspacePath,
  readCodeWorkspaceDocument,
  renameCodeWorkspacePath,
  type CodeWorkspaceMutationResult,
} from "../../shared/api/codeWorkspace";
import { errorToMessage } from "../../shared/utils/helpers";
import type { IdePreferences } from "./idePreferences";

export type ActiveWorkspaceDocumentForActions = {
  path: string;
  fingerprint: string;
  dirty: boolean;
};

export type WorkspaceActionTarget = {
  kind: "file" | "directory";
  path: string;
};

type ActionDialog =
  | { kind: "create-file"; value: string }
  | { kind: "create-directory"; value: string }
  | { kind: "rename"; target: WorkspaceActionTarget; value: string }
  | { kind: "delete"; target: WorkspaceActionTarget };

function parentPrefix(path: string | null | undefined): string {
  if (!path) return "";
  const normalized = path.replace(/\/+$/, "");
  return normalized ? `${normalized}/` : "";
}

export function useCodeWorkspaceActions({
  getOpenDocument,
  onMutation,
  onError,
  preferences,
}: {
  getOpenDocument: (path: string) => ActiveWorkspaceDocumentForActions | null;
  onMutation: (result: CodeWorkspaceMutationResult) => void | Promise<void>;
  onError: (message: string) => void;
  preferences: IdePreferences;
}) {
  const [dialog, setDialog] = useState<ActionDialog | null>(null);
  const [pending, setPending] = useState<string | null>(null);

  const run = async (label: string, task: () => Promise<CodeWorkspaceMutationResult>) => {
    if (pending) return;
    setPending(label);
    try {
      const result = await task();
      setDialog(null);
      await onMutation(result);
    } catch (error) {
      onError(errorToMessage(error));
    } finally {
      setPending(null);
    }
  };

  const resolveFingerprint = async (target: WorkspaceActionTarget): Promise<string | null> => {
    if (target.kind === "directory") return null;
    const open = getOpenDocument(target.path);
    if (open?.dirty) throw new Error(`Save or discard unsaved edits in ${target.path} first.`);
    if (open?.fingerprint) return open.fingerprint;
    const document = await readCodeWorkspaceDocument(target.path);
    return document.fingerprint;
  };

  const executeDelete = async (target: WorkspaceActionTarget) => {
    const fingerprint = await resolveFingerprint(target);
    await run("delete", () => deleteCodeWorkspacePath({
      path: target.path,
      expected_fingerprint: fingerprint,
    }));
  };

  return {
    pending,
    requestCreateFile(basePath?: string | null) {
      if (!pending) setDialog({ kind: "create-file", value: parentPrefix(basePath) });
    },
    requestCreateDirectory(basePath?: string | null) {
      if (!pending) setDialog({ kind: "create-directory", value: parentPrefix(basePath) });
    },
    requestRename(target: WorkspaceActionTarget) {
      const open = target.kind === "file" ? getOpenDocument(target.path) : null;
      if (open?.dirty) {
        onError(`Save or discard unsaved edits in ${target.path} before renaming it.`);
        return;
      }
      if (!pending) setDialog({ kind: "rename", target, value: target.path });
    },
    requestDelete(target: WorkspaceActionTarget) {
      const open = target.kind === "file" ? getOpenDocument(target.path) : null;
      if (open?.dirty) {
        onError(`Save or discard unsaved edits in ${target.path} before deleting it.`);
        return;
      }
      if (pending) return;
      if (preferences.confirmDelete) setDialog({ kind: "delete", target });
      else void executeDelete(target).catch((error) => onError(errorToMessage(error)));
    },
    dialog: (
      <CodeWorkspaceActionDialog
        dialog={dialog}
        pending={pending}
        onCancel={() => !pending && setDialog(null)}
        onSubmit={(value) => {
          if (!dialog) return;
          if (dialog.kind === "create-file") {
            void run("file", () => createCodeWorkspaceFile({ path: value }));
            return;
          }
          if (dialog.kind === "create-directory") {
            void run("folder", () => createCodeWorkspaceDirectory(value));
            return;
          }
          if (dialog.kind === "rename") {
            if (value === dialog.target.path) {
              setDialog(null);
              return;
            }
            void (async () => {
              try {
                const fingerprint = await resolveFingerprint(dialog.target);
                await run("rename", () => renameCodeWorkspacePath({
                  path: dialog.target.path,
                  new_path: value,
                  expected_fingerprint: fingerprint,
                }));
              } catch (error) {
                onError(errorToMessage(error));
              }
            })();
            return;
          }
          void executeDelete(dialog.target).catch((error) => onError(errorToMessage(error)));
        }}
      />
    ),
  };
}

function CodeWorkspaceActionDialog({
  dialog,
  pending,
  onCancel,
  onSubmit,
}: {
  dialog: ActionDialog | null;
  pending: string | null;
  onCancel: () => void;
  onSubmit: (value: string) => void;
}) {
  const [value, setValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!dialog) return;
    setValue("value" in dialog ? dialog.value : "");
    const timer = window.setTimeout(() => inputRef.current?.focus(), 0);
    return () => window.clearTimeout(timer);
  }, [dialog]);

  useEffect(() => {
    if (!dialog) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !pending) {
        event.preventDefault();
        onCancel();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [dialog, onCancel, pending]);

  if (!dialog) return null;
  const destructive = dialog.kind === "delete";
  const title = dialog.kind === "create-file"
    ? "New file"
    : dialog.kind === "create-directory"
      ? "New folder"
      : dialog.kind === "rename"
        ? "Rename or move"
        : "Delete from workspace";
  const target = "target" in dialog ? dialog.target.path : null;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const next = destructive ? "" : value.trim();
    if (!destructive && !next) return;
    onSubmit(next);
  };

  return (
    <div className="ide-dialog-backdrop" role="presentation" onMouseDown={(event) => {
      if (event.target === event.currentTarget && !pending) onCancel();
    }}>
      <form className="ide-dialog" role="dialog" aria-modal="true" aria-labelledby="code-action-dialog-title" onSubmit={submit}>
        <div className="ide-dialog-head">
          <strong id="code-action-dialog-title">{title}</strong>
          <span>Repository-relative paths only</span>
        </div>
        {destructive ? (
          <p className="ide-dialog-message">
            Delete <code>{target}</code>? RepoDesk will revalidate the exact path before the mutation.
          </p>
        ) : (
          <label className="ide-dialog-field">
            <span>{dialog.kind === "rename" ? "New path" : "Path"}</span>
            <input
              ref={inputRef}
              value={value}
              onChange={(event) => setValue(event.target.value)}
              spellCheck={false}
              autoComplete="off"
            />
          </label>
        )}
        <div className="ide-dialog-actions">
          <button type="button" className="ghost-button" onClick={onCancel} disabled={Boolean(pending)}>Cancel</button>
          <button type="submit" className={destructive ? "danger-button" : "primary-button"} disabled={Boolean(pending)}>
            {pending ? "Working…" : destructive ? "Delete" : dialog.kind === "rename" ? "Rename" : "Create"}
          </button>
        </div>
      </form>
    </div>
  );
}
