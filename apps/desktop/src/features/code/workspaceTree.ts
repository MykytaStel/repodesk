import type { CodeWorkspaceFile, CodeWorkspaceFileStatus } from "../../shared/api/codeWorkspace";

export type CodeTreeDirectory = {
  kind: "directory";
  name: string;
  path: string;
  children: CodeTreeNode[];
  changed: number;
};

export type CodeTreeFile = {
  kind: "file";
  name: string;
  path: string;
  file: CodeWorkspaceFile;
};

export type CodeTreeNode = CodeTreeDirectory | CodeTreeFile;
export type CodeTreeRow = { node: CodeTreeNode; depth: number };

const MAX_SEARCH_RESULTS = 300;

function isChanged(status: CodeWorkspaceFileStatus): boolean {
  return status !== "clean";
}

export function buildCodeTree(files: CodeWorkspaceFile[]): CodeTreeNode[] {
  const root: CodeTreeDirectory = {
    kind: "directory",
    name: "",
    path: "",
    children: [],
    changed: 0,
  };
  const directories = new Map<string, CodeTreeDirectory>([["", root]]);

  for (const file of files) {
    const parts = file.path.split("/").filter(Boolean);
    if (parts.length === 0) continue;
    let parent = root;
    let currentPath = "";

    for (const part of parts.slice(0, -1)) {
      currentPath = currentPath ? `${currentPath}/${part}` : part;
      let directory = directories.get(currentPath);
      if (!directory) {
        directory = {
          kind: "directory",
          name: part,
          path: currentPath,
          children: [],
          changed: 0,
        };
        directories.set(currentPath, directory);
        parent.children.push(directory);
      }
      if (isChanged(file.status)) directory.changed += 1;
      parent = directory;
    }

    parent.children.push({
      kind: "file",
      name: parts[parts.length - 1],
      path: file.path,
      file,
    });
  }

  const sort = (nodes: CodeTreeNode[]) => {
    nodes.sort((left, right) => {
      if (left.kind !== right.kind) return left.kind === "directory" ? -1 : 1;
      return left.name.localeCompare(right.name, undefined, { numeric: true, sensitivity: "base" });
    });
    for (const node of nodes) if (node.kind === "directory") sort(node.children);
  };
  sort(root.children);
  return root.children;
}

export function flattenCodeTree(nodes: CodeTreeNode[], expanded: Set<string>): CodeTreeRow[] {
  const rows: CodeTreeRow[] = [];
  const visit = (items: CodeTreeNode[], depth: number) => {
    for (const node of items) {
      rows.push({ node, depth });
      if (node.kind === "directory" && expanded.has(node.path)) visit(node.children, depth + 1);
    }
  };
  visit(nodes, 0);
  return rows;
}

export function searchCodeTree(files: CodeWorkspaceFile[], query: string): CodeTreeRow[] {
  const normalized = query.trim().toLocaleLowerCase();
  if (!normalized) return [];
  return files
    .filter((file) => file.path.toLocaleLowerCase().includes(normalized))
    .sort((left, right) => {
      const leftBase = left.name.toLocaleLowerCase().startsWith(normalized) ? 0 : 1;
      const rightBase = right.name.toLocaleLowerCase().startsWith(normalized) ? 0 : 1;
      return leftBase - rightBase || left.path.length - right.path.length || left.path.localeCompare(right.path);
    })
    .slice(0, MAX_SEARCH_RESULTS)
    .map((file) => ({
      depth: 0,
      node: { kind: "file", name: file.name, path: file.path, file },
    }));
}
