import { useEffect, useMemo, useState } from "react";
import type { CodeWorkspaceFile, CodeWorkspaceFileStatus } from "../../shared/api/codeWorkspace";
import { buildCodeTree, flattenCodeTree, searchCodeTree } from "./workspaceTree";

const MAX_EDITOR_BYTES = 512 * 1024;

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
  for (let index = 1; index < parts.length; index += 1) {
    parents.push(parts.slice(0, index).join("/"));
  }
  return parents;
}

export function CodeWorkspaceTree({
  files,
  activePath,
  onOpen,
}: {
  files: CodeWorkspaceFile[];
  activePath: string | null;
  onOpen: (file: CodeWorkspaceFile) => void;
}) {
  const [query, setQuery] = useState("");
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set());
  const tree = useMemo(() => buildCodeTree(files), [files]);
  const rows = useMemo(
    () => query.trim() ? searchCodeTree(files, query) : flattenCodeTree(tree, expanded),
    [expanded, files, query, tree],
  );

  useEffect(() => {
    if (!activePath) return;
    setExpanded((current) => {
      const next = new Set(current);
      for (const parent of parentPaths(activePath)) next.add(parent);
      return next;
    });
  }, [activePath]);

  const toggleDirectory = (path: string) => {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  return (
    <aside className="code-explorer" aria-label="Repository explorer">
      <div className="code-explorer-head">
        <strong>Explorer</strong>
        <span>{files.length}</span>
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
      <div className="code-tree" role="tree" aria-label="Repository files">
        {rows.length === 0 ? <p className="code-tree-empty">No matching files.</p> : null}
        {rows.map(({ node, depth }) => {
          if (node.kind === "directory") {
            const open = expanded.has(node.path);
            return (
              <button
                type="button"
                key={`dir:${node.path}`}
                className="code-tree-row directory"
                style={{ paddingLeft: 8 + depth * 14 }}
                onClick={() => toggleDirectory(node.path)}
                role="treeitem"
                aria-expanded={open}
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
              style={{ paddingLeft: 24 + depth * 14 }}
              onClick={() => !unavailable && onOpen(file)}
              disabled={Boolean(unavailable)}
              title={unavailable ?? file.path}
              role="treeitem"
            >
              <span className="code-tree-file-icon" aria-hidden="true">·</span>
              <span className="code-tree-name">{file.name}</span>
              {unavailable ? <span className="code-tree-status neutral">lock</span> : null}
              {!unavailable && label ? (
                <span className={`code-tree-status ${statusTone(file.status)}`}>{label}</span>
              ) : null}
            </button>
          );
        })}
      </div>
      {query.trim() && rows.length >= 300 ? (
        <div className="code-explorer-foot">Showing the first 300 matches.</div>
      ) : null}
    </aside>
  );
}
