import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useGit } from "../git/useGit";
import { useCode } from "../code/useCode";
import { FindingRow, HealthTrend } from "../code/CodeFindings";
import { DiffViewer } from "../../shared/ui/DiffViewer";
import { ActorBadge, EmptyState, stringifyPreview } from "../../shared/ui/SharedComponents";
import { callCommand, queryKeys } from "../../shared/api/queries";
import { requestCodeWorkspaceOpen } from "../../shared/api/codeWorkspace";
import {
  WORK_ENGINEERING_SNAPSHOT_KEY,
  workEngineeringSnapshot,
  type ChangeFileScopeState,
} from "../../shared/api/engineering";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import type { TabId } from "../../shared/types/api";
import { listFromRecord } from "../../shared/utils/helpers";
import type { FileFindings } from "../../shared/api/repopilot";
import { ChangeGovernancePanel } from "./ChangeGovernancePanel";

// "Changes" is the evidence surface between produced code and a commit. Git
// state still drives what is physically present in the checkout; the engineering
// ledger adds provenance, scope, review and verification without pretending an
// unrecorded working-tree edit belongs to the latest agent ChangeSet.
type FileStatus = "staged" | "modified" | "untracked";
const STATUS_META: Record<FileStatus, { label: string; tone: string }> = {
  staged: { label: "Staged", tone: "ok" },
  modified: { label: "Modified", tone: "warn" },
  untracked: { label: "New", tone: "" },
};

type ViewMode = "diff" | "file";

function scopeMeta(state: ChangeFileScopeState): { label: string; tone: string } {
  switch (state) {
    case "allowed": return { label: "In scope", tone: "ok" };
    case "out_of_scope": return { label: "Out of scope", tone: "danger" };
    case "protected": return { label: "Protected", tone: "danger" };
    case "ungoverned": return { label: "Ungoverned", tone: "warn" };
  }
}

export function ChangesTab({
  setActiveTab,
}: {
  setActiveTab: (tab: TabId, detail?: string) => void;
}) {
  const queryClient = useQueryClient();
  const { hasTask } = useWorkspace();
  const { git, branch, dirty, dirtyCount } = useGit();
  const { changedFiles, report, fileFindings, reviewing, runReview, trend } = useCode();
  const engineering = useQuery({
    queryKey: WORK_ENGINEERING_SNAPSHOT_KEY,
    queryFn: () => workEngineeringSnapshot(),
    enabled: hasTask,
    refetchInterval: 4_000,
  });

  const [selectedFile, setSelectedFile] = useState("");
  const [preview, setPreview] = useState("");
  const [previewLoading, setPreviewLoading] = useState(false);
  const [viewMode, setViewMode] = useState<ViewMode>("diff");
  const autoRan = useRef(false);

  const staged = listFromRecord(git, ["staged", "staged_files"]);
  const unstaged = listFromRecord(git, ["unstaged", "unstaged_files", "modified_files"]);
  const untracked = listFromRecord(git, ["untracked", "untracked_files"]);
  const governance = engineering.data?.change_governance ?? null;

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

  const governanceByFile = useMemo(() => {
    const map = new Map<string, ChangeFileScopeState>();
    for (const file of governance?.files ?? []) map.set(file.path, file.scope_state);
    return map;
  }, [governance]);

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
        const cached = staged.includes(path);
        let result = await callCommand<string>("git_file_diff", { path, cached });
        if (!result.trim() && !cached) result = await callCommand<string>("git_file_diff", { path, cached: true });
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

  useEffect(() => {
    if (selectedFile) void loadPreview(selectedFile, viewMode);
  }, [viewMode]);

  useEffect(() => {
    if (!autoRan.current && rows.length > 0 && !report && !reviewing) {
      autoRan.current = true;
      runReview();
    }
  }, [rows.length, report, reviewing, runReview]);

  const refreshWorkspace = () => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.git.snapshot });
    void queryClient.invalidateQueries({ queryKey: WORK_ENGINEERING_SNAPSHOT_KEY });
  };

  const openSelectedInCode = () => {
    if (!selectedFile) return;
    requestCodeWorkspaceOpen(selectedFile);
    setActiveTab("code", `Open ${selectedFile} in the guarded editor.`);
  };

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

      {hasTask ? (
        <ChangeGovernancePanel
          governance={governance}
          loading={engineering.isLoading}
          error={engineering.isError ? engineering.error : null}
        />
      ) : (
        <div className="change-evidence-message">
          <strong>No active Work Item</strong>
          <span>Git changes are visible, but RepoDesk cannot attribute or govern them until a Work Item is active.</span>
        </div>
      )}

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
                const scope = governanceByFile.get(file);
                const physicallyChanged = status != null || changedFiles.includes(file);
                const unattributed = physicallyChanged && governance?.changeset_id != null && scope == null;
                const scopeBadge = scope ? scopeMeta(scope) : null;
                return (
                  <button
                    key={file}
                    className={`file-row ${active ? "active" : ""}`}
                    onClick={() => void loadPreview(file, viewMode)}
                    onDoubleClick={() => {
                      requestCodeWorkspaceOpen(file);
                      setActiveTab("code", `Open ${file} in Code.`);
                    }}
                  >
                    <code>{file}</code>
                    <span className="file-badges">
                      {scopeBadge ? <span className={`pill ${scopeBadge.tone}`}>{scopeBadge.label}</span> : null}
                      {unattributed ? <span className="pill warn">Unattributed</span> : null}
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
                    <button className="tiny-button ghost-button" onClick={openSelectedInCode}>Open in Code</button>
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
