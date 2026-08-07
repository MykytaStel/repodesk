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
  workingProviders: number;
  providerCount: number;
  totalTokens: number | null | undefined;
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
      return "Inspect scope, context, execution, verification and completion evidence for the active Work Item.";
    case "code":
      return "File- and symbol-level selection will extend this inspector. Algorithmic Profile v0 already lives in core.";
    case "changes":
      return "Use Changes to review the workspace delta; context coverage below shows whether changed files were prepared for the worker.";
    case "history":
      return "Runs aggregates execution history. This inspector keeps the current Work Item evidence visible while you move through history.";
    case "projects":
      return "Project-level knowledge, rules and repository intelligence will attach to this inspector in later slices.";
    default:
      return "This panel is intentionally read-only. Feature-specific inspectors can attach here without moving domain logic into React.";
  }
}

export function WorkspaceInspector({
  activeTab,
  projectName,
  taskTitle,
  hasTask,
  dirty,
  dirtyCount,
  workingProviders,
  providerCount,
  totalTokens,
  onNavigate,
}: WorkspaceInspectorProps) {
  const snapshot = useQuery({
    queryKey: WORK_ENGINEERING_SNAPSHOT_KEY,
    queryFn: workEngineeringSnapshot,
    enabled: hasTask,
    refetchInterval: 4_000,
  });

  const report = snapshot.data?.intelligence;
  const context = snapshot.data?.context_inspector;
  const manifest = context?.manifest;
  const coverage = context?.file_evidence.latest;

  return (
    <aside className="workspace-inspector" aria-label="Inspector">
      <div className="workspace-inspector-scroll">
        <header className="workspace-inspector-heading">
          <p className="eyebrow">Inspector</p>
          <h2>{hasTask ? taskTitle : "Workspace"}</h2>
          <p>{activeViewHint(activeTab)}</p>
        </header>

        <section className="inspector-section">
          <span className="inspector-section-label">Workspace</span>
          <InspectorMetric label="Project" value={projectName || "Not connected"} detail={dirty ? `${dirtyCount} uncommitted changes` : "Git working tree clean"} />
          <InspectorMetric label="Models" value={`${workingProviders}/${providerCount}`} detail="reachable providers" />
          <InspectorMetric label="AI tokens" value={totalTokens == null ? "—" : totalTokens.toLocaleString()} detail="recorded workspace usage" />
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
                detail={manifest ? `${manifest.excluded_files} excluded · ${manifest.included_file_tokens.toLocaleString()} tokens` : "build context to create a manifest"}
              />
              <InspectorMetric
                label="Change coverage"
                value={coverage?.change_coverage == null ? "—" : `${Math.round(coverage.change_coverage * 100)}%`}
                detail={coverage ? `${coverage.changed_files_present_in_context.length}/${coverage.changed_files.length} changed files prepared` : "known after a changeset follows context"}
              />
            </section>

            <section className="inspector-section">
              <span className="inspector-section-label">Engineering evidence</span>
              <InspectorMetric
                label="Execution"
                value={report ? `${report.execution.completed}/${report.execution.attempts}` : "—"}
                detail={report ? `${report.execution.unique_workers} workers · ${report.execution.handoffs} handoffs` : "loading evidence"}
              />
              <InspectorMetric
                label="Changesets"
                value={report ? `${report.changes.accepted_changesets} accepted` : "—"}
                detail={report ? `${report.changes.pending_review_changesets} pending · ${report.changes.rejected_changesets} rejected` : "loading evidence"}
              />
              <InspectorMetric
                label="Verification"
                value={report ? `${report.verification.passed}/${report.verification.finished}` : "—"}
                detail={report ? `${report.verification.failed} failed · ${report.verification.commands_run} commands` : "loading evidence"}
              />
            </section>
          </>
        )}
      </div>

      <footer className="workspace-inspector-footer">
        <button type="button" className="workspace-inspector-action" onClick={() => onNavigate("work", "Opened full Work Item evidence.")}>Full Work evidence</button>
      </footer>
    </aside>
  );
}
