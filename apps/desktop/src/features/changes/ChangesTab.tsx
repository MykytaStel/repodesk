import { useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useGit } from "../git/useGit";
import { useCode } from "../code/useCode";
import { FindingRow, HealthTrend } from "../code/CodeFindings";
import { DiffViewer } from "../../shared/ui/DiffViewer";
import { EmptyState, stringifyPreview } from "../../shared/ui/SharedComponents";
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

type FileStatus = "staged" | "modified" | "untracked";
const STATUS_META: Record<FileStatus, { label: string; tone: string }> = {
  staged: { label: "S", tone: "ok" },
  modified: { label: "M", tone: "warn" },
  untracked: { label: "U", tone: "neutral" },
};

type ViewMode = "diff" | "file";

function exceptionalScopeMeta(state: ChangeFileScopeState | undefined): { label: string; tone: string } | null {
  switch (state) {
    case "out_of_scope": return { label: "Out of scope", tone: "danger" };
    case "protected": return { label: "Protected", tone: "danger" };
    case "ungoverned": return { label: "Ungoverned", tone: "warn" };
    default: return null;
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
  const [findingsOpen, setFindingsOpen] = useState(false);
  const { changedFiles, report, fileFindings, reviewing, runReview, trend } = useCode({ includeHistory: findingsOpen });
  const engineering = useQuery({
    queryKey: WORK_ENGINEERING_SNAPSHOT_KEY,
    queryFn: () => workEngineeringSnapshot(),
    enabled: hasTask,
    staleTime: 2_000,
    refetchOnWindowFocus: true,
  });

  const [selectedFile, setSelectedFile] = useState("");
  const [preview, setPreview] = useState("");
  const [previewLoading, setPreviewLoading] = useState(false);
  const [viewMode, setViewMode] = useState<ViewMode>("diff");
  const [evidenceOpen, setEvidenceOpen] = useState(false);

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
  const blocker = governance?.gate.blockers[0] ?? null;
  const gateLabel = governance?.gate.ready
    ? "Ready to commit"
    : governance?.gate.state?.split("_").join(" ") ?? "No ChangeSet";

  return (
    <div className="changes-tab changes-focus-layout">
      <header className="changes-focus-header">
        <div className="changes-focus-title">
          <p className="eyebrow">Changes</p>
          <strong>{branch}</strong>
          <span className={dirty ? "warn" : "muted"}>{dirty ? `${dirtyCount} uncommitted` : "Working tree clean"}</span>
        </div>
        <div className="changes-focus-actions">
          <button className="tiny-button" onClick={refreshWorkspace}>Refresh</button>
          <button className="tiny-button" onClick={() => setEvidenceOpen((open) => !open)}>
            Evidence{governance?.gate.ready ? " ✓" : blocker ? " !" : ""}
          </button>
          <button
            className={`tiny-button${findingsOpen ? " active" : ""}`}
            onClick={() => {
              if (!report && !reviewing) runReview();
              setFindingsOpen((open) => !open);
            }}
            disabled={reviewing}
          >
            {reviewing ? "Analyzing…" : report ? `Findings ${report.total}` : "Analyze"}
          </button>
        </div>
      </header>

      {hasTask ? (
        <div className={`changes-gate-bar${blocker ? " danger" : ""}`}>
          <span>Commit gate</span>
          <strong>{gateLabel}</strong>
          {blocker ? <small>{blocker}</small> : <small>{governance?.changeset_id ?? "No active ChangeSet"}</small>}
          <button className="link-cta" onClick={() => setEvidenceOpen((open) => !open)}>
            {evidenceOpen ? "Hide evidence" : "Inspect evidence"}
          </button>
        </div>
      ) : (
        <div className="changes-gate-bar muted">
          <span>Governance</span><strong>No active Work Item</strong><small>Git changes are visible but unattributed.</small>
        </div>
      )}

      {evidenceOpen && hasTask ? (
        <ChangeGovernancePanel
          governance={governance}
          loading={engineering.isLoading}
          error={engineering.isError ? engineering.error : null}
        />
      ) : null}

      <div className="changes-focus-workspace">
        <section className="changes-file-pane" aria-label="Changed files">
          <div className="changes-pane-head">
            <strong>Files</strong>
            <span>{rows.length}</span>
          </div>
          <div className="file-list scroll-area changes-file-list">
            {rows.length === 0 ? <p className="muted">No changed files.</p> : null}
            {rows.map((file) => {
              const status = statusOf(file);
              const group = findingsByFile.get(file);
              const active = file === selectedFile;
              const scope = governanceByFile.get(file);
              const exceptionalScope = exceptionalScopeMeta(scope);
              const physicallyChanged = status != null || changedFiles.includes(file);
              const unattributed = physicallyChanged && governance?.changeset_id != null && scope == null;
              return (
                <button
                  key={file}
                  className={`file-row changes-file-row${active ? " active" : ""}`}
                  onClick={() => void loadPreview(file, viewMode)}
                  onDoubleClick={() => {
                    requestCodeWorkspaceOpen(file);
                    setActiveTab("code", `Open ${file} in Code.`);
                  }}
                >
                  <code>{file}</code>
                  <span className="file-badges">
                    {exceptionalScope ? <span className={`pill ${exceptionalScope.tone}`}>{exceptionalScope.label}</span> : null}
                    {unattributed ? <span className="pill warn">Unattributed</span> : null}
                    {status ? <span className={`pill ${STATUS_META[status].tone}`}>{STATUS_META[status].label}</span> : null}
                    {group?.blocking ? <span className="pill danger">{group.blocking}</span> : null}
                  </span>
                </button>
              );
            })}
          </div>
        </section>

        <section className="changes-preview-pane">
          <div className="changes-pane-head preview">
            <div className="changes-preview-location">
              <strong>{selectedFile ? selectedFile.split("/").pop() : "Diff"}</strong>
              {selectedFile ? <code>{selectedFile}</code> : <span>Select a changed file</span>}
            </div>
            {selectedFile ? (
              <div className="changes-preview-actions">
                <button className="tiny-button" onClick={openSelectedInCode}>Open in Code</button>
                <div className="changes-view-switch" role="group" aria-label="Changes view">
                  <button className={`tiny-button${viewMode === "diff" ? " active" : ""}`} onClick={() => setViewMode("diff")}>Diff</button>
                  <button className={`tiny-button${viewMode === "file" ? " active" : ""}`} onClick={() => setViewMode("file")}>File</button>
                </div>
              </div>
            ) : null}
          </div>

          <div className="changes-preview-content">
            {!selectedFile ? (
              <EmptyState message={dirty ? "Select a file to inspect its diff." : "Working tree is clean."} hint="Double-click a file to edit it in Code." />
            ) : previewLoading ? (
              <p className="muted">Loading…</p>
            ) : viewMode === "diff" ? (
              <DiffViewer diff={preview} />
            ) : (
              <pre className="code-panel scrollable">{preview || "No content."}</pre>
            )}
          </div>
        </section>

        {findingsOpen ? (
          <aside className="changes-findings-drawer" aria-label="RepoPilot findings">
            <div className="changes-pane-head">
              <div>
                <strong>Engineering findings</strong>
                <span>{report ? `${report.total} in current changes` : "Not analyzed"}</span>
              </div>
              <button className="tiny-button" onClick={() => setFindingsOpen(false)}>×</button>
            </div>
            {report?.error ? <div className="notice danger">{report.error}</div> : null}
            {report ? <HealthTrend points={trend} /> : null}
            {!report ? (
              <p className="muted">Run Analyze to inspect the current diff.</p>
            ) : selectedGroup ? (
              <ul className="findings-list">
                {selectedGroup.findings.map((finding, index) => <FindingRow key={index} finding={finding} />)}
              </ul>
            ) : selectedFile ? (
              <p className="muted">No findings in this file.</p>
            ) : fileFindings.length === 0 ? (
              <p className="muted">No findings in the current diff.</p>
            ) : (
              fileFindings.slice(0, 12).map((group) => (
                <button className="changes-finding-group" key={group.file} onClick={() => void loadPreview(group.file, viewMode)}>
                  <code>{group.file}</code>
                  <span>{group.total}</span>
                </button>
              ))
            )}
          </aside>
        ) : null}
      </div>
    </div>
  );
}
