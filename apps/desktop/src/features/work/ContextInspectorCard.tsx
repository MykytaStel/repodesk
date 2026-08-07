import { useQuery } from "@tanstack/react-query";
import {
  WORK_ENGINEERING_SNAPSHOT_KEY,
  workEngineeringSnapshot,
  type ContextFileEntry,
} from "../../shared/api/engineering";
import { useWorkspace } from "../../shared/hooks/useWorkspace";

function percent(value: number | null): string {
  return value == null ? "—" : `${Math.round(value * 100)}%`;
}

function exclusionLabel(entry: ContextFileEntry): string {
  if (entry.status === "included") {
    const tokens = entry.included_tokens?.toLocaleString() ?? "unknown";
    return `${tokens} tokens${entry.trimmed ? " · trimmed" : ""}`;
  }
  return entry.exclusion_reason?.replace(/_/g, " ") ?? "excluded";
}

export function ContextInspectorCard() {
  const { hasTask } = useWorkspace();
  const snapshot = useQuery({
    queryKey: WORK_ENGINEERING_SNAPSHOT_KEY,
    queryFn: workEngineeringSnapshot,
    enabled: hasTask,
    refetchInterval: 4000,
  });

  if (!hasTask) return null;

  if (snapshot.isError) {
    return (
      <div className="content-grid">
        <section className="panel wide-panel">
          <p className="eyebrow">Context Inspector</p>
          <p className="notice danger">Could not load context evidence: {String(snapshot.error)}</p>
        </section>
      </div>
    );
  }

  if (snapshot.isLoading || !snapshot.data) {
    return (
      <div className="content-grid">
        <section className="panel wide-panel">
          <p className="eyebrow">Context Inspector</p>
          <p className="muted">Loading bounded context…</p>
        </section>
      </div>
    );
  }

  const report = snapshot.data.context_inspector;
  const manifest = report.manifest;
  const compactness = report.compactness.latest;
  const coverage = report.file_evidence.latest;

  if (!manifest) {
    return (
      <div className="content-grid">
        <section className="panel wide-panel" aria-label="Context Inspector">
          <p className="eyebrow">Context Inspector</p>
          <h3>Bounded repository context</h3>
          <p className="muted">
            Build context to create a manifest. RepoDesk will only include repository files explicitly referenced as backtick paths in the Work Item Scope.
          </p>
        </section>
      </div>
    );
  }

  return (
    <div className="content-grid">
      <section className="panel wide-panel" aria-label="Context Inspector">
        <div className="phase-brief-head">
          <div>
            <p className="eyebrow">Context Inspector</p>
            <h3>What the worker can actually see</h3>
          </div>
          <span className="pill accent">manifest v{manifest.version}</span>
        </div>

        <p className="muted">
          File content is selected from explicit Work Item scope. Excluded files remain visible as evidence but their content is not injected.
        </p>

        <div className="phase-brief-grid">
          <div className="phase-brief-cell">
            <span>Included files</span>
            <strong>{manifest.included_files}</strong>
            <small className="muted">{manifest.included_file_tokens.toLocaleString()} file-content tokens</small>
          </div>
          <div className="phase-brief-cell">
            <span>Excluded files</span>
            <strong>{manifest.excluded_files}</strong>
            <small className="muted">ignored, unsafe, missing, or outside budget</small>
          </div>
          <div className="phase-brief-cell">
            <span>Context kept</span>
            <strong>{percent(compactness?.compactness_ratio ?? null)}</strong>
            <small className="muted">
              {compactness ? `${compactness.included_tokens.toLocaleString()} / ${compactness.candidate_tokens.toLocaleString()} candidate tokens` : "no compactness evidence"}
            </small>
          </div>
          <div className="phase-brief-cell">
            <span>Repeated context</span>
            <strong>{percent(compactness?.repeated_context_ratio ?? null)}</strong>
            <small className="muted">unchanged component tokens vs previous measured build</small>
          </div>
          <div className="phase-brief-cell">
            <span>Change coverage</span>
            <strong>{percent(coverage?.change_coverage ?? null)}</strong>
            <small className="muted">
              {coverage ? `${coverage.changed_files_present_in_context.length}/${coverage.changed_files.length} changed files were in context` : "available after a changeset follows this context build"}
            </small>
          </div>
          <div className="phase-brief-cell">
            <span>Compared changesets</span>
            <strong>{report.file_evidence.compared_changesets}</strong>
            <small className="muted">ledger-backed context/change comparisons</small>
          </div>
        </div>

        <div className="phase-brief-head">
          <div>
            <p className="eyebrow">Scoped files</p>
            <h3>Selection evidence</h3>
          </div>
        </div>

        {manifest.entries.length === 0 ? (
          <p className="muted">No explicit file paths were found in the Work Item Scope.</p>
        ) : (
          <div className="phase-brief-grid">
            {manifest.entries.map((entry) => (
              <div className="phase-brief-cell" key={entry.path}>
                <span>{entry.status === "included" ? "Included" : "Excluded"}</span>
                <strong>{entry.path}</strong>
                <small className="muted">task scope · {exclusionLabel(entry)}</small>
              </div>
            ))}
          </div>
        )}

        {coverage && coverage.changed_files_missing_from_context.length > 0 ? (
          <p className="notice warning">
            Changed outside prepared context: {coverage.changed_files_missing_from_context.join(", ")}
          </p>
        ) : null}
      </section>
    </div>
  );
}
