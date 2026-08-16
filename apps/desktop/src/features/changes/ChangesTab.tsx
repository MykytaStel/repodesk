import { useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useGit } from "../git/useGit";
import { useCode } from "../code/useCode";
import { FindingRow, HealthTrend } from "../code/CodeFindings";
import { DiffViewer } from "../../shared/ui/DiffViewer";
import { stringifyPreview } from "../../shared/ui/SharedComponents";
import {
  ActionBar,
  EmptyState,
  ErrorState,
  EvidenceState,
  LoadingState,
  PanelHeader,
  StatusBadge,
} from "../../shared/ui/primitives";
import { callCommand, queryKeys } from "../../shared/api/queries";
import { requestCodeWorkspaceOpen } from "../../shared/api/codeWorkspace";
import { consumeChangesOpenRequest } from "../../shared/api/changesNavigation";
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
import {
  fileScopeSemantic,
  fileStatusSemantic,
  safeCommitSemantic,
  type ChangeFileStatus,
} from "./changesSemantic";
import "./changes-route.css";
import "../routing/routing-feature.css";

type ViewMode = "diff" | "file";

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
  const [pendingFocus, setPendingFocus] = useState<string | null>(() => consumeChangesOpenRequest());
  const [navigationWarning, setNavigationWarning] = useState<string | null>(null);

  const staged = listFromRecord(git, ["staged", "staged_files"]);
  const unstaged = listFromRecord(git, ["unstaged", "unstaged_files", "modified_files"]);
  const untracked = listFromRecord(git, ["untracked", "untracked_files"]);
  const governance = engineering.data?.change_governance ?? null;
  const passport = engineering.data?.changeset_passport ?? null;
  const manifest = engineering.data?.safe_commit_manifest ?? null;

  const statusOf = (file: string): ChangeFileStatus | null =>
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
    if (rows.length === 0) return;

    if (pendingFocus) {
      const requested = pendingFocus;
      const target = rows.includes(requested) ? requested : rows[0];
      setPendingFocus(null);
      setViewMode("diff");
      setNavigationWarning(
        target === requested
          ? null
          : `${requested} has no current Git delta. Showing ${target} instead.`,
      );
      void loadPreview(target, "diff");
      return;
    }

    if (!selectedFile || !rows.includes(selectedFile)) {
      setNavigationWarning(null);
      void loadPreview(rows[0], viewMode);
    }
    // Selection changes are deliberately driven by the changed-file identity set.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rows, pendingFocus]);

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
  const safeCommit = manifest ? safeCommitSemantic(manifest) : null;
  const blocker = manifest?.blockers[0] ?? null;

  return (
    <div className="changes-tab changes-focus-layout">
      <div className="changes-focus-header">
        <PanelHeader
          eyebrow="Changes"
          title={branch}
          description={dirty ? `${dirtyCount} uncommitted` : "Clean workspace"}
          trailing={(
            <div className="changes-focus-actions">
              <button className="tiny-button" onClick={refreshWorkspace}>Refresh</button>
              <button className="tiny-button" onClick={() => setEvidenceOpen((open) => !open)}>
                Manifest{manifest?.state === "ready" || manifest?.state === "committed" ? " ✓" : blocker ? " !" : ""}
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
          )}
        />
      </div>

      {hasTask ? (
        <div className="changes-gate-bar">
          <EvidenceState
            label="Safe commit"
            state={safeCommit?.label ?? "No ChangeSet"}
            tone={safeCommit?.tone ?? "neutral"}
            detail={blocker ?? (manifest?.manifest_digest
              ? `Manifest ${manifest.manifest_digest.slice(0, 12)}`
              : governance?.changeset_id ?? "No active ChangeSet")}
          />
          <button className="link-cta" onClick={() => setEvidenceOpen((open) => !open)}>
            {evidenceOpen ? "Hide manifest" : "Inspect manifest"}
          </button>
        </div>
      ) : (
        <div className="changes-gate-bar">
          <EvidenceState label="Governance" state="No Work Item" tone="attention" detail="Changes are not attributed to a task." />
        </div>
      )}

      {navigationWarning ? (
        <div className="notice warning" role="status">
          {navigationWarning}
          <button type="button" className="link-cta" onClick={() => setNavigationWarning(null)}>Dismiss</button>
        </div>
      ) : null}

      {evidenceOpen && hasTask ? (
        <ChangeGovernancePanel
          governance={governance}
          passport={passport}
          manifest={manifest}
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
            {rows.length === 0 ? (
              <EmptyState message="No changes" hint="This project has no uncommitted files." />
            ) : null}
            {rows.map((file) => {
              const status = statusOf(file);
              const group = findingsByFile.get(file);
              const active = file === selectedFile;
              const scope = governanceByFile.get(file);
              const exceptionalScope = scope && scope !== "allowed" ? fileScopeSemantic(scope) : null;
              const physicallyChanged = status != null || changedFiles.includes(file);
              const unattributed = physicallyChanged && governance?.changeset_id != null && scope == null;
              const fileStatus = status ? fileStatusSemantic(status) : null;
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
                    {exceptionalScope ? <StatusBadge label={exceptionalScope.label} tone={exceptionalScope.tone} /> : null}
                    {unattributed ? <StatusBadge label="Unattributed" tone="critical" /> : null}
                    {fileStatus ? <StatusBadge label={fileStatus.label} tone={fileStatus.tone} ariaLabel={fileStatus.detail} /> : null}
                    {group?.blocking ? <StatusBadge label={String(group.blocking)} tone="critical" ariaLabel={`${group.blocking} blocking findings`} /> : null}
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
              {selectedFile ? <code>{selectedFile}</code> : <span>{dirty ? "Select a file" : "No file selected"}</span>}
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
              <EmptyState
                message={dirty ? "Select a file" : "Nothing to review"}
                hint={dirty ? "Choose a changed file to inspect its diff." : "New edits will appear here after refresh."}
              />
            ) : previewLoading ? (
              <LoadingState message="Loading file preview…" />
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
            {report?.error ? <ErrorState title="Analysis unavailable" detail={report.error} /> : null}
            {report ? <HealthTrend points={trend} /> : null}
            {!report ? (
              <EmptyState message="Not analyzed" hint="Run Analyze to inspect the current diff." />
            ) : selectedGroup ? (
              <ul className="findings-list">
                {selectedGroup.findings.map((finding, index) => <FindingRow key={index} finding={finding} />)}
              </ul>
            ) : selectedFile ? (
              <EmptyState message="No findings" hint="This file has no findings in the current analysis." />
            ) : fileFindings.length === 0 ? (
              <EmptyState message="No findings" hint="The current diff has no findings." />
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
