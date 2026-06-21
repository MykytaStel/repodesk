import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { FileGroup, DiffView, stringifyPreview, EmptyState, getString } from "../../shared/ui/SharedComponents";
import { callCommand, queryKeys } from "../../shared/api/queries";
import { useGit } from "./useGit";
import { listFromRecord } from "../../shared/utils/helpers";

export function GitTab() {
  const queryClient = useQueryClient();
  const { git, branch, dirty, dirtyCount, isLoading: isBusy } = useGit();
  const gitDiffStat = getString(git, "diff_stat", "");

  const [selectedFile, setSelectedFile] = useState<string>("");
  const [diff, setDiff] = useState<string>("");
  const [diffLoading, setDiffLoading] = useState(false);

  const staged = listFromRecord(git, ["staged", "staged_files"]);

  const refreshAll = () => {
    queryClient.invalidateQueries({ queryKey: queryKeys.git.snapshot });
  };

  async function showDiff(file: string) {
    setSelectedFile(file);
    setDiffLoading(true);
    try {
      // Staged files use the cached diff; everything else the working-tree diff.
      const cached = staged.includes(file);
      const result = await callCommand<string>("git_file_diff", { path: file, cached });
      setDiff(result ?? "");
    } catch {
      setDiff("");
    } finally {
      setDiffLoading(false);
    }
  }

  return (
    <div className="content-grid two-column-grid">
      <div className="left-column">
        <section className="hero-panel">
          <p className="eyebrow">Workspace Git</p>
          <h1 className="branch-heading">{branch}</h1>
          <p className="lead">{dirty ? `${dirtyCount} uncommitted changes.` : "Working tree clean."} Click a file to see its diff; commit via terminal before using agents.</p>
          <div className="button-row">
            <button className="primary-button" onClick={() => void refreshAll()} disabled={isBusy}>Refresh workspace</button>
          </div>
        </section>
        <div className="file-group-stack">
          <FileGroup title="Staged" files={staged} onSelect={(f) => void showDiff(f)} activeFile={selectedFile} />
          <FileGroup title="Unstaged" files={listFromRecord(git, ["unstaged", "unstaged_files", "modified_files"])} onSelect={(f) => void showDiff(f)} activeFile={selectedFile} />
          <FileGroup title="Untracked" files={listFromRecord(git, ["untracked", "untracked_files"])} />
        </div>
      </div>
      <div className="right-column">
        <section className="panel fill-height">
          <div className="panel-title-row"><p className="eyebrow">{selectedFile ? "Diff" : "Diff stat"}</p>{selectedFile && <code>{selectedFile}</code>}</div>
          {selectedFile ? (
            diffLoading ? (
              <p className="muted">Loading diff…</p>
            ) : diff ? (
              <DiffView diff={diff} />
            ) : (
              <EmptyState message="No diff for this file." hint="It may be untracked or unchanged." />
            )
          ) : gitDiffStat ? (
            <pre className="code-panel scrollable">{gitDiffStat}</pre>
          ) : (
            <EmptyState message={dirty ? "Select a changed file to view its diff." : "Working tree is clean."} hint="Changed files appear on the left." />
          )}
          <details className="dev-details">
            <summary>Developer details (raw snapshot)</summary>
            <pre className="code-panel scrollable">{git ? stringifyPreview(git, 4000) : "No Git data loaded."}</pre>
          </details>
        </section>
      </div>
    </div>
  );
}
