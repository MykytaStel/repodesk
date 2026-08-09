import { useQuery } from "@tanstack/react-query";
import {
  WORK_ENGINEERING_SNAPSHOT_KEY,
  workEngineeringSnapshot,
} from "../shared/api/engineering";
import type { TabId } from "../shared/types/api";

interface WorkspaceInspectorProps {
  activeTab: TabId;
  projectName: string;
  taskTitle: string;
  hasTask: boolean;
  dirty: boolean;
  dirtyCount: number;
  onNavigate: (tab: TabId, detail?: string) => void;
}

function InspectorMetric({ label, value, detail }: { label: string; value: string; detail: string }) {
  return (
    <div className="inspector-metric">
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{detail}</small>
    </div>
  );
}

function activeViewHint(activeTab: TabId): string {
  switch (activeTab) {
    case "work":
      return "Current Work Item evidence without leaving the focused task canvas.";
    case "code":
      return "Repository context for the current code view.";
    case "changes":
      return "Scope and verification evidence behind the current changeset.";
    case "history":
      return "Current Work Item state while you inspect historical runs.";
    case "memory":
      return "Accepted Project Knowledge can enter future bounded context packs.";
    case "projects":
      return "Project-level engineering state and reusable knowledge live here.";
    default:
      return "Read-only engineering context for the current surface.";
  }
}

export function WorkspaceInspector({
  activeTab,
  projectName,
  taskTitle,
  hasTask,
  dirty,
  dirtyCount,
  onNavigate,
}: WorkspaceInspectorProps) {
  const snapshot = useQuery({
    queryKey: WORK_ENGINEERING_SNAPSHOT_KEY,
    queryFn: workEngineeringSnapshot,
    enabled: hasTask,
    staleTime: 3_000,
    refetchOnWindowFocus: true,
  });

  const report = snapshot.data?.intelligence;
  const context = snapshot.data?.context_inspector;
  const manifest = context?.manifest;
  const coverage = context?.file_evidence.latest;
  const governance = snapshot.data?.change_governance;

  return (
    <aside className="workspace-inspector" aria-label="Inspector">
      <div className="workspace-inspector-scroll">
        <header className="workspace-inspector-heading">
          <p className="eyebrow">Inspector</p>
          <h2>{hasTask ? taskTitle : "Workspace"}</h2>
          <p>{activeViewHint(activeTab)}</p>
        </header>

        <section className="inspector-section">
          <span className="inspector-section-label">Repository</span>
          <InspectorMetric
            label="Project"
            value={projectName || "Not connected"}
            detail={dirty ? `${dirtyCount} uncommitted changes` : "Git working tree clean"}
          />
        </section>

        {!hasTask ? (
          <section className="inspector-section">
            <p className="workspace-sidebar-empty">Create or select a Work Item to populate engineering evidence.</p>
            <button type="button" className="workspace-inspector-action" onClick={() => onNavigate("work")}>Open Work</button>
          </section>
        ) : snapshot.isError ? (
          <section className="inspector-section">
            <p className="notice danger">Engineering evidence unavailable: {String(snapshot.error)}</p>
          </section>
        ) : (
          <>
            <section className="inspector-section">
              <span className="inspector-section-label">Context</span>
              <InspectorMetric
                label="Prepared files"
                value={manifest ? `${manifest.included_files}` : "—"}
                detail={manifest ? `${manifest.excluded_files} excluded · ${manifest.included_file_tokens.toLocaleString()} tokens` : "Build context to create a manifest"}
              />
              <InspectorMetric
                label="Change coverage"
                value={coverage?.change_coverage == null ? "—" : `${Math.round(coverage.change_coverage * 100)}%`}
                detail={coverage ? `${coverage.changed_files_present_in_context.length}/${coverage.changed_files.length} changed files prepared` : "Known after a changeset follows context"}
              />
            </section>

            <section className="inspector-section">
              <span className="inspector-section-label">Current gate</span>
              <InspectorMetric
                label="Review"
                value={governance?.review_state ?? "—"}
                detail={governance?.changeset_id ? governance.changeset_id : "No current changeset"}
              />
              <InspectorMetric
                label="Verification"
                value={governance?.verification.state ?? "—"}
                detail={governance ? `${governance.verification.command_count} recorded commands` : "Loading evidence"}
              />
              <InspectorMetric
                label="Commit"
                value={governance?.gate.ready ? "Ready" : governance?.gate.state ?? "—"}
                detail={governance?.gate.blockers[0] ?? (governance?.committed ? "Committed" : "No active blocker")}
              />
            </section>

            <section className="inspector-section">
              <span className="inspector-section-label">History</span>
              <InspectorMetric
                label="Executions"
                value={report ? `${report.execution.completed}/${report.execution.attempts}` : "—"}
                detail={report ? `${report.execution.unique_workers} workers · ${report.execution.handoffs} handoffs` : "Loading evidence"}
              />
              <InspectorMetric
                label="Verification"
                value={report ? `${report.verification.passed}/${report.verification.finished}` : "—"}
                detail={report ? `${report.verification.failed} failed across ${report.verification.commands_run} commands` : "Loading evidence"}
              />
            </section>
          </>
        )}
      </div>

      <footer className="workspace-inspector-footer">
        <button type="button" className="workspace-inspector-action" onClick={() => onNavigate("memory", "Opened reviewed Project Knowledge.")}>Project Knowledge</button>
        <button type="button" className="workspace-inspector-action" onClick={() => onNavigate("work", "Opened full Work Item evidence.")}>Open Work</button>
      </footer>
    </aside>
  );
}
