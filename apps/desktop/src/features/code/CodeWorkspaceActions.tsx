import { useState } from "react";
import {
  createCodeWorkspaceDirectory,
  createCodeWorkspaceFile,
  deleteCodeWorkspacePath,
  renameCodeWorkspacePath,
  type CodeWorkspaceMutationResult,
} from "../../shared/api/codeWorkspace";
import { errorToMessage } from "../../shared/utils/helpers";

export type ActiveWorkspaceDocumentForActions = {
  path: string;
  fingerprint: string;
  dirty: boolean;
};

export function CodeWorkspaceActions({
  activeDocument,
  onMutation,
  onError,
}: {
  activeDocument: ActiveWorkspaceDocumentForActions | null;
  onMutation: (result: CodeWorkspaceMutationResult) => void | Promise<void>;
  onError: (message: string) => void;
}) {
  const [pending, setPending] = useState<string | null>(null);

  const run = async (label: string, task: () => Promise<CodeWorkspaceMutationResult>) => {
    if (pending) return;
    setPending(label);
    try {
      const result = await task();
      await onMutation(result);
    } catch (error) {
      onError(errorToMessage(error));
    } finally {
      setPending(null);
    }
  };

  const createFile = () => {
    const path = window.prompt("New file path (relative to the repository)", "src/")?.trim();
    if (!path) return;
    void run("file", () => createCodeWorkspaceFile({ path }));
  };

  const createDirectory = () => {
    const path = window.prompt("New folder path (relative to the repository)", "src/")?.trim();
    if (!path) return;
    void run("folder", () => createCodeWorkspaceDirectory(path));
  };

  const rename = () => {
    if (!activeDocument || activeDocument.dirty) return;
    const nextPath = window.prompt("Rename or move to repository-relative path", activeDocument.path)?.trim();
    if (!nextPath || nextPath === activeDocument.path) return;
    void run("rename", () => renameCodeWorkspacePath({
      path: activeDocument.path,
      new_path: nextPath,
      expected_fingerprint: activeDocument.fingerprint,
    }));
  };

  const remove = () => {
    if (!activeDocument || activeDocument.dirty) return;
    const confirmed = window.confirm(
      `Delete ${activeDocument.path}?\n\nRepoDesk will only delete this exact reviewed file.`,
    );
    if (!confirmed) return;
    void run("delete", () => deleteCodeWorkspacePath({
      path: activeDocument.path,
      expected_fingerprint: activeDocument.fingerprint,
    }));
  };

  const destructiveDisabled = !activeDocument || activeDocument.dirty || Boolean(pending);
  const destructiveTitle = activeDocument?.dirty
    ? "Save or discard unsaved edits before rename/delete"
    : undefined;

  return (
    <div className="code-workspace-file-actions" role="group" aria-label="Repository file actions">
      <button type="button" className="tiny-button" onClick={createFile} disabled={Boolean(pending)}>
        {pending === "file" ? "Creating…" : "New file"}
      </button>
      <button type="button" className="tiny-button" onClick={createDirectory} disabled={Boolean(pending)}>
        {pending === "folder" ? "Creating…" : "New folder"}
      </button>
      <button
        type="button"
        className="tiny-button"
        onClick={rename}
        disabled={destructiveDisabled}
        title={destructiveTitle}
      >
        {pending === "rename" ? "Renaming…" : "Rename"}
      </button>
      <button
        type="button"
        className="tiny-button danger"
        onClick={remove}
        disabled={destructiveDisabled}
        title={destructiveTitle}
      >
        {pending === "delete" ? "Deleting…" : "Delete"}
      </button>
    </div>
  );
}
