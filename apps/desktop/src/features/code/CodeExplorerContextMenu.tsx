import { useEffect, useRef } from "react";
import { IdeIcon, type IdeIconName } from "./IdeIcon";
import type { WorkspaceActionTarget } from "./CodeWorkspaceActions";

export type ExplorerContextMenuState = {
  x: number;
  y: number;
  target: WorkspaceActionTarget;
};

type MenuAction = {
  label: string;
  icon: IdeIconName;
  danger?: boolean;
  disabled?: boolean;
  run: () => void;
};

export function CodeExplorerContextMenu({
  state,
  onClose,
  onNewFile,
  onNewFolder,
  onRename,
  onDelete,
  onCopyPath,
  onRefresh,
}: {
  state: ExplorerContextMenuState | null;
  onClose: () => void;
  onNewFile: (basePath: string | null) => void;
  onNewFolder: (basePath: string | null) => void;
  onRename: (target: WorkspaceActionTarget) => void;
  onDelete: (target: WorkspaceActionTarget) => void;
  onCopyPath: (path: string) => void;
  onRefresh: () => void;
}) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!state) return;
    const close = (event: MouseEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) onClose();
    };
    const keydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener("mousedown", close, true);
    window.addEventListener("keydown", keydown);
    const timer = window.setTimeout(() => menuRef.current?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus(), 0);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("mousedown", close, true);
      window.removeEventListener("keydown", keydown);
    };
  }, [onClose, state]);

  if (!state) return null;
  const basePath = state.target.kind === "directory"
    ? state.target.path
    : state.target.path.includes("/")
      ? state.target.path.slice(0, state.target.path.lastIndexOf("/"))
      : null;
  const actions: MenuAction[] = [
    { label: "New File", icon: "file-add", run: () => onNewFile(basePath) },
    { label: "New Folder", icon: "folder-add", run: () => onNewFolder(basePath) },
    { label: "Rename…", icon: "rename", run: () => onRename(state.target) },
    { label: "Delete", icon: "delete", danger: true, run: () => onDelete(state.target) },
    { label: "Copy Relative Path", icon: "copy", run: () => onCopyPath(state.target.path) },
    { label: "Refresh Explorer", icon: "refresh", run: onRefresh },
  ];

  return (
    <div
      ref={menuRef}
      className="ide-context-menu"
      role="menu"
      aria-label={`Explorer actions for ${state.target.path}`}
      style={{ left: Math.min(state.x, window.innerWidth - 230), top: Math.min(state.y, window.innerHeight - 260) }}
      onContextMenu={(event) => event.preventDefault()}
    >
      <div className="ide-context-menu-path" title={state.target.path}>{state.target.path}</div>
      {actions.map((action, index) => (
        <button
          key={action.label}
          type="button"
          role="menuitem"
          className={`ide-context-menu-item${action.danger ? " danger" : ""}${index === 4 ? " separated" : ""}`}
          disabled={action.disabled}
          onClick={() => {
            onClose();
            action.run();
          }}
        >
          <IdeIcon name={action.icon} />
          <span>{action.label}</span>
        </button>
      ))}
    </div>
  );
}
