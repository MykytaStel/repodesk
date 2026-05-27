import React from "react";
import { statusTone, FileGroup, stringifyPreview } from "../../shared/ui/SharedComponents";
import { useGit } from "./useGit";
import { listFromRecord } from "../../shared/utils/helpers";

export function GitTab() {
  const { git, branch, dirty, dirtyCount, isLoading: isBusy } = useGit();
  const refreshAll = () => {};

  return (
    <div className="content-grid two-column-grid">
      <div className="left-column">
        <section className="hero-panel">
          <p className="eyebrow">Workspace Git</p>
          <h1>Branch {branch}</h1>
          <p className="lead">{dirty ? `${dirtyCount} uncommitted changes.` : "Working tree clean."} Commit via terminal before using agents.</p>
          <div className="button-row">
            <button className="primary-button" onClick={() => void refreshAll()} disabled={isBusy}>Refresh workspace</button>
          </div>
        </section>
        <section className="panel">
          <FileGroup title="Staged" files={listFromRecord(git, ["staged", "staged_files"])} />
          <FileGroup title="Unstaged" files={listFromRecord(git, ["unstaged", "unstaged_files", "modified_files"])} />
          <FileGroup title="Untracked" files={listFromRecord(git, ["untracked", "untracked_files"])} />
        </section>
      </div>
      <div className="right-column">
        <section className="panel fill-height preview-panel">
          <div className="panel-title-row"><p className="eyebrow">Git diagnostic</p></div>
          <pre className="code-panel scrollable">{git ? stringifyPreview(git, 4000) : "No Git data loaded."}</pre>
        </section>
      </div>
    </div>
  );
}
