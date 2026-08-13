import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CodeWorkspaceFile, CodeWorkspaceFileStatus } from "../../shared/api/codeWorkspace";
import { CodeExplorerContextMenu, type ExplorerContextMenuState } from "./CodeExplorerContextMenu";
import { IdeIcon } from "./IdeIcon";
import { useIdePreferences } from "./idePreferences";
import type { WorkspaceActionTarget } from "./CodeWorkspaceActions";
import { buildCodeTree, flattenCodeTree, searchCodeTree } from "./workspaceTree";

const MAX_EDITOR_BYTES = 512 * 1024;
const VIRTUALIZE_AFTER = 120;
const OVERSCAN_ROWS = 8;
const DEFAULT_VIEWPORT_HEIGHT = 520;

const STATUS_LABEL: Record<CodeWorkspaceFileStatus, string> = {
  clean: "",
  modified: "M",
  added: "A",
  deleted: "D",
  untracked: "U",
  renamed: "R",
  conflict: "!",
};

function statusTone(status: CodeWorkspaceFileStatus): string {
  if (status === "conflict") return "danger";
  if (status === "modified" || status === "untracked") return "warn";
  if (status === "added") return "ok";
  return "neutral";
}

function unavailableReason(file: CodeWorkspaceFile): string | null {
  if (file.blocked) return "Unavailable for safe text editing by RepoDesk policy";
  if (file.status === "deleted") return "File is deleted from the working tree";
  if (file.bytes > MAX_EDITOR_BYTES) return "File exceeds the 512 KiB editor budget";
  return null;
}

function parentPaths(path: string): string[] {
  const parts = path.split("/");
  const parents: string[] = [];
  for (let index = 1; index < parts.length; index += 1) parents.push(parts.slice(0, index).join("/"));
  return parents;
}

export function CodeWorkspaceTree({
  files,
  activePath,
  onOpen,
  onSearchProject,
  onNewFile,
  onNewFolder,
  onRename,
  onDelete,
  onRefresh,
}: {
  files: CodeWorkspaceFile[];
  activePath: string | null;
  onOpen: (file: CodeWorkspaceFile) => void;
  onSearchProject: () => void;
  onNewFile: (basePath?: string | null) => void;
  onNewFolder: (basePath?: string | null) => void;
  onRename: (target: WorkspaceActionTarget) => void;
  onDelete: (target: WorkspaceActionTarget) => void;
  onRefresh: () => void;
}) {
  const preferences = useIdePreferences();
  const rowHeight = preferences.explorerDensity === "compact" ? 23 : 27;
  const [query, setQuery] = useState("");
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set());
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(DEFAULT_VIEWPORT_HEIGHT);
  const [contextMenu, setContextMenu] = useState<ExplorerContextMenuState | null>(null);
  const treeRef = useRef<HTMLDivElement>(null);
  const tree = useMemo(() => buildCodeTree(files), [files]);
  const rows = useMemo(
    () => query.trim() ? searchCodeTree(files, query) : flattenCodeTree(tree, expanded),
    [expanded, files, query, tree],
  );
  const virtualized = rows.length > VIRTUALIZE_AFTER;
  const windowRange = useMemo(() => {
    if (!virtualized) return { start: 0, end: rows.length };
    const visibleRows = Math.ceil(viewportHeight / rowHeight);
    const start = Math.max(0, Math.floor(scrollTop / rowHeight) - OVERSCAN_ROWS);
    const end = Math.min(rows.length, start + visibleRows + OVERSCAN_ROWS * 2);
    return { start, end };
  }, [rowHeight, rows.length, scrollTop, viewportHeight, virtualized]);
  const visibleRows = useMemo(
    () => rows.slice(windowRange.start, windowRange.end).map((row, offset) => ({ ...row, flatIndex: windowRange.start + offset })),
    [rows, windowRange.end, windowRange.start],
  );
  const topSpacer = virtualized ? windowRange.start * rowHeight : 0;
  const bottomSpacer = virtualized ? (rows.length - windowRange.end) * rowHeight : 0;

  useEffect(() => {
    const element = treeRef.current;
    if (!element) return;
    const updateViewport = () => {
      const nextHeight = element.clientHeight;
      if (nextHeight > 0) setViewportHeight(nextHeight);
    };
    updateViewport();
    const observer = new ResizeObserver(updateViewport);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!activePath) return;
    setExpanded((current) => {
      const next = new Set(current);
      let changed = false;
      for (const parent of parentPaths(activePath)) {
        if (!next.has(parent)) {
          next.add(parent);
          changed = true;
        }
      }
      return changed ? next : current;
    });
  }, [activePath]);

  useEffect(() => {
    const element = treeRef.current;
    if (!element || !activePath || query.trim()) return;
    const index = rows.findIndex(({ node }) => node.kind === "file" && node.file.path === activePath);
    if (index < 0) return;
    const rowTop = index * rowHeight;
    const rowBottom = rowTop + rowHeight;
    const viewportTop = element.scrollTop;
    const viewportBottom = viewportTop + element.clientHeight;
    let nextScrollTop = viewportTop;
    if (rowTop < viewportTop) nextScrollTop = rowTop;
    else if (rowBottom > viewportBottom) nextScrollTop = Math.max(0, rowBottom - element.clientHeight);
    if (nextScrollTop !== viewportTop) {
      element.scrollTop = nextScrollTop;
      setScrollTop(nextScrollTop);
    }
  }, [activePath, query, rowHeight, rows]);

  useEffect(() => {
    const element = treeRef.current;
    if (!element) return;
    element.scrollTop = 0;
    setScrollTop(0);
  }, [query]);

  const toggleDirectory = (path: string) => {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  const openContextMenu = useCallback((event: React.MouseEvent, target: WorkspaceActionTarget, blocked = false) => {
    event.preventDefault();
    event.stopPropagation();
    setContextMenu({ x: event.clientX, y: event.clientY, target, blocked });
  }, []);

  const copyRelativePath = useCallback((path: string) => {
    void navigator.clipboard?.writeText(path).catch(() => undefined);
  }, []);

  return (
    <aside className="code-explorer" aria-label="Repository explorer">
      <div className="code-explorer-head">
        <div className="code-explorer-title"><strong>Explorer</strong><span>{files.length}</span></div>
        <div className="ide-icon-toolbar" role="toolbar" aria-label="Explorer actions">
          <IconAction label="Search project" icon="search" onClick={onSearchProject} />
          <IconAction label="New file" icon="file-add" onClick={() => onNewFile(null)} />
          <IconAction label="New folder" icon="folder-add" onClick={() => onNewFolder(null)} />
          <IconAction label="Refresh Explorer" icon="refresh" onClick={onRefresh} />
          <IconAction label="Collapse folders" icon="collapse" onClick={() => setExpanded(new Set())} />
        </div>
      </div>
      <div className="code-explorer-search">
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Filter files…"
          aria-label="Filter repository files"
          spellCheck={false}
        />
        {query ? <button type="button" onClick={() => setQuery("")} aria-label="Clear file filter">×</button> : null}
      </div>
      <div
        ref={treeRef}
        className="code-tree"
        role="tree"
        aria-label="Repository files"
        data-virtualized={virtualized ? "true" : "false"}
        data-total-rows={rows.length}
        onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
        onContextMenu={(event) => event.preventDefault()}
      >
        {rows.length === 0 ? <p className="code-tree-empty">No matching files.</p> : null}
        {topSpacer > 0 ? <div aria-hidden="true" style={{ height: topSpacer }} /> : null}
        {visibleRows.map(({ node, depth, flatIndex }) => {
          const position = flatIndex + 1;
          if (node.kind === "directory") {
            const open = expanded.has(node.path);
            return (
              <button
                type="button"
                key={`dir:${node.path}`}
                className="code-tree-row directory"
                style={{ paddingLeft: 8 + depth * 14, height: rowHeight }}
                onClick={() => toggleDirectory(node.path)}
                onContextMenu={(event) => openContextMenu(event, { kind: "directory", path: node.path })}
                role="treeitem"
                aria-expanded={open}
                aria-posinset={position}
                aria-setsize={rows.length}
              >
                <span className="code-tree-chevron" aria-hidden="true">{open ? "⌄" : "›"}</span>
                <span className="code-tree-name">{node.name}</span>
                {node.changed > 0 ? <span className="code-tree-count">{node.changed}</span> : null}
              </button>
            );
          }

          const { file } = node;
          const active = file.path === activePath;
          const label = STATUS_LABEL[file.status];
          const unavailable = unavailableReason(file);
          return (
            <button
              type="button"
              key={`file:${file.path}`}
              className={`code-tree-row file${active ? " active" : ""}${unavailable ? " blocked" : ""}`}
              style={{ paddingLeft: 24 + depth * 14, height: rowHeight }}
              onClick={() => !unavailable && onOpen(file)}
              onContextMenu={(event) => openContextMenu(event, { kind: "file", path: file.path }, Boolean(file.blocked))}
              disabled={Boolean(unavailable)}
              title={unavailable ?? file.path}
              role="treeitem"
              aria-posinset={position}
              aria-setsize={rows.length}
            >
              <span className="code-tree-file-icon" aria-hidden="true">·</span>
              <span className="code-tree-name">{file.name}</span>
              {unavailable ? <span className="code-tree-status neutral">lock</span> : null}
              {!unavailable && label ? <span className={`code-tree-status ${statusTone(file.status)}`}>{label}</span> : null}
            </button>
          );
        })}
        {bottomSpacer > 0 ? <div aria-hidden="true" style={{ height: bottomSpacer }} /> : null}
      </div>
      {query.trim() && rows.length >= 300 ? <div className="code-explorer-foot">Showing the first 300 matches.</div> : null}
      <CodeExplorerContextMenu
        state={contextMenu}
        onClose={() => setContextMenu(null)}
        onNewFile={onNewFile}
        onNewFolder={onNewFolder}
        onRename={onRename}
        onDelete={onDelete}
        onCopyPath={copyRelativePath}
        onRefresh={onRefresh}
      />
    </aside>
  );
}

function IconAction({
  label,
  icon,
  onClick,
}: {
  label: string;
  icon: Parameters<typeof IdeIcon>[0]["name"];
  onClick: () => void;
}) {
  return (
    <button type="button" className="ide-icon-button" aria-label={label} title={label} onClick={onClick}>
      <IdeIcon name={icon} />
    </button>
  );
}
