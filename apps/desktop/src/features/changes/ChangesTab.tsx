import { useEffect, useMemo, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useGit } from "../git/useGit";
import { useCode } from "../code/useCode";
import { FindingRow, HealthTrend } from "../code/CodeFindings";
import { DiffViewer } from "../../shared/ui/DiffViewer";
import { ActorBadge, EmptyState, stringifyPreview } from "../../shared/ui/SharedComponents";
import { callCommand, queryKeys } from "../../shared/api/queries";
import { listFromRecord } from "../../shared/utils/helpers";
import type { FileFindings } from "../../shared/api/repopilot";

// "Changes" is the single surface between an agent's output and a commit: one
// changed-files list (git status + RepoPilot findings) feeding one preview pane
// (diff or full file) with inline findings — no segmented Git/Code detour.
type FileStatus = "staged" | "modified" | "untracked";
const STATUS_META: Record<FileStatus, { label: string; tone: string }> = {
  staged: { label: "Staged", tone: "ok" },
  modified: { label: "Modified", tone: "warn" },
  untracked: { label: "New", tone: "" },
};
type ViewMode = "diff" | "file";

export function ChangesTab() {
  const queryClient = useQueryClient();
  const { git, branch, dirty, dirtyCount } = useGit();
  const { changedFiles, report, fileFindings, reviewing, runReview, trend } = useCode();

  const [selectedFile, setSelectedFile] = useState("");
  const [preview, setPreview] = useState("");
  const [previewLoading, setPreviewLoading] = useState(false);
  const [viewMode, setViewMode] = useState<ViewMode>("diff");
  const autoRan = useRef(false);

  const staged = listFromRecord(git, ["staged", "staged_files"]);
  const unstaged = listFromRecord(git, ["unstaged", "unstaged_files", "modified_files"]);
  const untracked = listFromRecord(git, ["untracked", "untracked_files"]);

  const statusOf = (file: string): FileStatus | null =>
    staged.includes(file)
      ? "staged"
      : untracked.includes(file)
        ? "untracked"
        : unstaged.includes(file)
          ? "modified"
          : null;

  const findingsByFile = useMemo(() => {
    const map = new Map<string, FileFindings>();
    for (const group of fileFindings) map.set(group.file, group);
    return map;
  }, [fileFindings]);

  // One list: every file touched in the working tree or flagged by RepoPilot,
  // worst findings first, then alphabetical.
  const rows = useMemo(() => {
    const set = new Set<string>([...changedFiles, ...staged, ...unstaged, ...untracked]);
    for (const group of fileFindings) set.add(group.file);
    return [...set].sort(
      (a, b) =>
        (findingsByFile.get(b)?.worstRank ?? 0) - (findingsByFile.get(a)?.worstRank ?? 0) || a.localeCompare(b),
    );
  }, [changedFiles, staged, unstaged, untracked, fileFindings, findingsByFile]);

  async function loadPreview(path: string, mode: ViewMode) {
    setSelectedFile(path);
    setPreviewLoading(true);
    try {
      if (mode === "diff") {
        // Staged files use the cached diff; for the rest fall back to it if the
        // working-tree diff is empty (e.g. already staged edits).
        const cached = staged.includes(path);
        let result = await callCommand<string>("git_file_diff", { path, cached });
        if (!result.trim() && !cached) {
          result = await callCommand<string>("git_file_diff", { path, cached: true });
        }
        setPreview(result.trim() ? result : "");
      } else {
        const result = await callCommand<any>("read_code_file", { relativePath: path, relative_path: path });
        setPreview(result?.content ?? stringifyPreview(result));
      }
    } catch (error) {
      setPreview(String(error));
    } finally {
      setPreviewLoading(false);
    }
  }

  // Re-load when toggling diff/full-file for the already-selected file.
  useEffect(() => {
    if (selectedFile) void loadPreview(selectedFile, viewMode);
  }, [viewMode]);

  // Auto-run a review once when the tab opens with changes to inspect.
  useEffect(() => {
    if (!autoRan.current && rows.length > 0 && !report && !reviewing) {
      autoRan.current = true;
      runReview();
    }
  }, [rows.length, report, reviewing, runReview]);

  const refreshWorkspace = () => queryClient.invalidateQueries({ queryKey: queryKeys.git.snapshot });
  const selectedGroup = selectedFile ? findingsByFile.get(selectedFile) : undefined;
  const counts = report?.counts;

  return (
    <div className="changes-tab">
      <div className="changes-summary">
        <div>
          <p className="eyebrow">Changes</p>
          <strong>{branch}</strong>
        </div>
        <div className="changes-actions">
          <span className={`changes-pill ${dirty ? "dirty" : "clean"}`}>
            {dirty ? `${dirtyCount} uncommitted` : "Working tree clean"}
          </span>
          <button className="ghost-button" onClick={refreshWorkspace}>Refresh</button>
          <button className="primary-button" onClick={() => runReview()} disabled={reviewing}>
            {reviewing ? "Reviewing…" : report ? "Re-run RepoPilot" : "Run RepoPilot"}
          </button>
          <ActorBadge mode="auto" />
        </div>
      </div>

      {report?.error ? (
        <div className="notice danger">{report.error}</div>
      ) : report ? (
        <div className="changes-counts">
          <div className="route-summary-grid">
            <div><span>Critical</span><strong>{counts?.critical ?? 0}</strong></div>
            <div><span>High</span><strong>{counts?.high ?? 0}</strong></div>
            <div><span>Medium</span><strong>{counts?.medium ?? 0}</strong></div>
            <div><span>Low</span><strong>{counts?.low ?? 0}</strong></div>
          </div>
          <HealthTrend points={trend} />
        </div>
      ) : null}

      <div className="content-grid two-column-grid">
        <div className="left-column">
          <section className="panel">
            <div className="panel-title-row compact">
              <h2>Changed files</h2>
              <span className="pill">{rows.length}</span>
            </div>
            <div className="file-list scroll-area small">
              {rows.length === 0 && <p className="muted">No changed files. Working tree is clean.</p>}
              {rows.map((file) => {
                const status = statusOf(file);
                const group = findingsByFile.get(file);
                const active = file === selectedFile;
                return (
                  <button
                    key={file}
                    className={`file-row ${active ? "active" : ""}`}
                    onClick={() => void loadPreview(file, viewMode)}
                  >
                    <code>{file}</code>
                    <span className="file-badges">
                      {status && <span className={`pill ${STATUS_META[status].tone}`}>{STATUS_META[status].label}</span>}
                      {group && group.blocking > 0 && <span className="pill danger">{group.blocking}</span>}
                      {group && <span className="pill">{group.total}</span>}
                    </span>
                  </button>
                );
              })}
            </div>
          </section>
        </div>

        <div className="right-column">
          <section className="panel preview-panel">
            <div className="panel-title-row">
              <p className="eyebrow">{selectedFile ? "Preview" : "Diff"}</p>
              {selectedFile && (
                <div className="preview-controls">
                  <code>{selectedFile}</code>
                  <div className="button-row" style={{ marginTop: 0 }}>
                    <button className={`tiny-button ${viewMode === "diff" ? "primary-button" : "ghost-button"}`} onClick={() => setViewMode("diff")}>Diff</button>
                    <button className={`tiny-button ${viewMode === "file" ? "primary-button" : "ghost-button"}`} onClick={() => setViewMode("file")}>Full File</button>
                  </div>
                </div>
              )}
            </div>
            {!selectedFile ? (
              <EmptyState message={dirty ? "Select a file to view its diff." : "Working tree is clean."} hint="Changed files appear on the left." />
            ) : previewLoading ? (
              <p className="muted">Loading…</p>
            ) : viewMode === "diff" ? (
              <DiffViewer diff={preview} />
            ) : (
              <pre className="code-panel scrollable">{preview || "No content."}</pre>
            )}
          </section>

          <section className="panel">
            <div className="panel-title-row">
              <p className="eyebrow">Findings</p>
              {selectedFile && <strong>{selectedFile}</strong>}
            </div>
            {!report ? (
              <p className="muted">Run RepoPilot to surface findings inline.</p>
            ) : selectedGroup ? (
              <ul className="findings-list">
                {selectedGroup.findings.map((finding, index) => <FindingRow key={index} finding={finding} />)}
              </ul>
            ) : selectedFile ? (
              <p className="muted">No findings in this file.</p>
            ) : fileFindings.length === 0 ? (
              <p className="muted">No findings in the current diff.</p>
            ) : (
              fileFindings.map((group) => (
                <div key={group.file} className="finding-group">
                  <div className="finding-group-head">
                    <code>{group.file}</code>
                    {group.blocking > 0 && <span className="pill danger">{group.blocking} blocking</span>}
                  </div>
                  <ul className="findings-list">
                    {group.findings.map((finding, index) => <FindingRow key={index} finding={finding} />)}
                  </ul>
                </div>
              ))
            )}
          </section>
        </div>
      </div>
    </div>
  );
}
