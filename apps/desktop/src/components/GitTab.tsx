import React from "react";
import { asRecord, getString, stringifyPreview, MetricCard, FileGroup } from "./SharedComponents";

interface GitTabProps {
  git: any;
  dirty: boolean;
  dirtyCount: number;
  branch: string;
  isBusy: boolean;
  refreshAll: (label: string) => void;
}

function listFromRecord(source: unknown, keys: string[]): string[] {
  const record = asRecord(source);
  for (const key of keys) {
    const value = record[key];
    if (Array.isArray(value)) {
      return value.map((item) => {
        if (typeof item === "string") return item;
        const itemRecord = asRecord(item);
        return getString(itemRecord, "path", getString(itemRecord, "name", stringifyPreview(item, 160)));
      });
    }
  }
  return [];
}

export function GitTab({
  git,
  dirty,
  dirtyCount,
  branch,
  isBusy,
  refreshAll,
}: GitTabProps) {
  const staged = listFromRecord(git, ["staged", "staged_files"]);
  const unstaged = listFromRecord(git, ["unstaged", "unstaged_files", "modified_files"]);
  const untracked = listFromRecord(git, ["untracked", "untracked_files"]);
  const diffStat = getString(git, "diff_stat", getString(git, "stat", "No diff stat available"));

  return (
    <div className="content-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Git</p>
        <h1>{dirty ? `${dirtyCount} pending changes` : "Workspace clean"}</h1>
        <p className="lead">Read-only workspace view. RepoDesk does not stage, commit, reset, or push from this screen.</p>
        <button className="ghost-button" onClick={() => void refreshAll("Refreshing Git")} disabled={isBusy}>Refresh Git</button>
      </section>
      <MetricCard label="Branch" value={branch} detail={`Last commit: ${getString(git, "last_commit", "-")}`} />
      <MetricCard label="Staged" value={String(staged.length)} detail="Ready for commit" />
      <MetricCard label="Unstaged" value={String(unstaged.length)} detail="Modified but not staged" tone={unstaged.length ? "warn" : "ok"} />
      <MetricCard label="Untracked" value={String(untracked.length)} detail="New files" tone={untracked.length ? "warn" : "ok"} />
      <section className="panel wide-panel"><p className="eyebrow">Diff stat</p><pre className="code-panel">{diffStat}</pre></section>
      <FileGroup title="Staged files" files={staged} />
      <FileGroup title="Unstaged files" files={unstaged} />
      <FileGroup title="Untracked files" files={untracked} />
    </div>
  );
}
