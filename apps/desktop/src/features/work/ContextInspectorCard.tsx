import { useQuery } from "@tanstack/react-query";
import {
  WORK_ENGINEERING_SNAPSHOT_KEY,
  workEngineeringSnapshot,
  type ContextFileEntry,
  type ContextInspectorReport,
} from "../../shared/api/engineering";
import { useWorkspace } from "../../shared/hooks/useWorkspace";

type PipelineCandidate = {
  id: string;
  provenance: {
    kind: string;
    locator: string;
    fingerprint: string;
    observed_at?: string | null;
  };
  trust: string;
  candidate_tokens: number;
  required: boolean;
  relevance_score?: number | null;
  freshness_score?: number | null;
};

type PipelineSelection = {
  candidate_id: string;
  state: "included" | "excluded";
  included_tokens: number;
  trimmed: boolean;
  exclusion_reason?: string | null;
  order?: number | null;
};

type ContextPipelineSnapshot = {
  version: number;
  project: string;
  task_id: string;
  generated_at: string;
  token_budget?: number | null;
  candidate_tokens: number;
  included_tokens: number;
  context_fingerprint: string;
  candidates: PipelineCandidate[];
  selections: PipelineSelection[];
};

type ContextInspectorV2 = ContextInspectorReport & {
  pipeline?: ContextPipelineSnapshot | null;
  pipeline_error?: string | null;
};

function percent(value: number | null | undefined): string {
  return value == null ? "—" : `${Math.round(value * 100)}%`;
}

function exclusionLabel(entry: ContextFileEntry): string {
  if (entry.status === "included") {
    const tokens = entry.included_tokens?.toLocaleString() ?? "unknown";
    return `${tokens} tokens${entry.trimmed ? " · trimmed" : ""}`;
  }
  return entry.exclusion_reason?.replace(/_/g, " ") ?? "excluded";
}

function sourceLabel(kind: string): string {
  const labels: Record<string, string> = {
    project_metadata: "Project metadata",
    task_metadata: "Task metadata",
    task_document: "Task document",
    work_item_contract: "Work Item Contract",
    scoped_file: "Scoped repository files",
    engineering_knowledge: "Engineering Knowledge",
    legacy_memory: "Memory Brain",
    decision_log: "Decision log",
    risk_log: "Risk log",
    checks: "Configured checks",
    git_state: "Git working tree",
    repository_map: "Repository map",
    semantic_search: "Semantic search",
    agent_rules: "Agent rules",
    other: "Other context",
  };
  return labels[kind] ?? kind.replace(/_/g, " ");
}

function trustLabel(trust: string): string {
  return trust.replace(/_/g, " ");
}

function decisionLabel(candidate: PipelineCandidate, selection: PipelineSelection): string {
  if (selection.state === "excluded") {
    return selection.exclusion_reason?.replace(/_/g, " ") ?? "excluded";
  }
  if (candidate.required) return "required";
  if (candidate.relevance_score != null) return `relevance ${percent(candidate.relevance_score)}`;
  return "selected";
}

function shortFingerprint(value: string): string {
  if (value.length <= 16) return value;
  return `${value.slice(0, 8)}…${value.slice(-6)}`;
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
      <section className="context-evidence-shell context-evidence-error">
        <p className="eyebrow">Context Evidence</p>
        <h3>Context evidence unavailable</h3>
        <p>{String(snapshot.error)}</p>
      </section>
    );
  }

  if (snapshot.isLoading || !snapshot.data) {
    return (
      <section className="context-evidence-shell">
        <p className="eyebrow">Context Evidence</p>
        <p className="muted">Loading AI packet evidence…</p>
      </section>
    );
  }

  const report = snapshot.data.context_inspector as ContextInspectorV2;
  const pipeline = report.pipeline ?? null;
  const manifest = report.manifest;
  const compactness = report.compactness.latest;
  const coverage = report.file_evidence.latest;

  if (report.pipeline_error) {
    return (
      <section className="context-evidence-shell context-evidence-error" aria-label="Context Evidence">
        <div>
          <p className="eyebrow">Context Evidence</p>
          <h3>Pipeline evidence is damaged</h3>
        </div>
        <p className="muted">
          RepoDesk kept the Work surface available, but it will not invent context-selection evidence. Rebuild context from Prepare to replace this artifact.
        </p>
        <code>{report.pipeline_error}</code>
      </section>
    );
  }

  if (!pipeline) {
    return (
      <section className="context-evidence-shell" aria-label="Context Evidence">
        <div className="context-evidence-title">
          <div>
            <p className="eyebrow">Context Evidence</p>
            <h3>No AI packet has been built yet</h3>
          </div>
        </div>
        <p className="muted">
          Prepare the Work Item to let RepoDesk rank bounded sources, apply the token budget, and record why each source was included or excluded.
        </p>
      </section>
    );
  }

  const selections = new Map(pipeline.selections.map((selection) => [selection.candidate_id, selection]));
  const rows = pipeline.candidates
    .map((candidate) => ({ candidate, selection: selections.get(candidate.id) }))
    .filter((row): row is { candidate: PipelineCandidate; selection: PipelineSelection } => Boolean(row.selection))
    .sort((left, right) => {
      if (left.selection.state !== right.selection.state) {
        return left.selection.state === "included" ? -1 : 1;
      }
      if (left.selection.state === "included") {
        return (left.selection.order ?? Number.MAX_SAFE_INTEGER) - (right.selection.order ?? Number.MAX_SAFE_INTEGER);
      }
      return (right.candidate.relevance_score ?? 0) - (left.candidate.relevance_score ?? 0);
    });

  const included = rows.filter((row) => row.selection.state === "included").length;
  const excluded = rows.length - included;
  const savedTokens = Math.max(0, pipeline.candidate_tokens - pipeline.included_tokens);
  const budget = pipeline.token_budget ?? null;
  const budgetRatio = budget && budget > 0 ? Math.min(1, pipeline.included_tokens / budget) : null;

  return (
    <section className="context-evidence-shell" aria-label="Context Evidence">
      <header className="context-evidence-title">
        <div>
          <p className="eyebrow">Context Evidence</p>
          <h3>AI packet composition</h3>
          <p className="muted">What the worker receives, why it was selected, and what RepoDesk left out.</p>
        </div>
        <span className="context-evidence-version">pipeline v{pipeline.version}</span>
      </header>

      <div className="context-budget-block">
        <div className="context-budget-copy">
          <div>
            <span>Context budget</span>
            <strong>
              {pipeline.included_tokens.toLocaleString()}
              {budget ? ` / ${budget.toLocaleString()}` : ""} tokens
            </strong>
          </div>
          <span className="context-budget-percent">{percent(budgetRatio)}</span>
        </div>
        <div
          className="context-budget-track"
          role="progressbar"
          aria-label="Context token budget used"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={budgetRatio == null ? undefined : Math.round(budgetRatio * 100)}
        >
          <span style={{ width: `${Math.round((budgetRatio ?? 0) * 100)}%` }} />
        </div>
        <div className="context-evidence-facts">
          <span><strong>{included}</strong> included</span>
          <span><strong>{excluded}</strong> excluded</span>
          <span><strong>{savedTokens.toLocaleString()}</strong> tokens removed</span>
          <span><strong>{percent(compactness?.repeated_context_ratio)}</strong> repeated</span>
          <span><strong>{percent(coverage?.change_coverage)}</strong> change coverage</span>
        </div>
      </div>

      <div className="context-decision-list" role="list" aria-label="Context source decisions">
        <div className="context-decision-head" aria-hidden="true">
          <span>Source</span>
          <span>Decision</span>
          <span>Trust</span>
          <span>Fresh</span>
          <span>Tokens</span>
        </div>

        {rows.map(({ candidate, selection }) => (
          <details
            className={`context-decision-row ${selection.state}`}
            key={candidate.id}
            role="listitem"
          >
            <summary>
              <span className="context-source-cell">
                <i aria-hidden="true" />
                <span>
                  <strong>{sourceLabel(candidate.provenance.kind)}</strong>
                  <small>{candidate.provenance.locator}</small>
                </span>
              </span>
              <span className="context-decision-cell">{decisionLabel(candidate, selection)}</span>
              <span className={`context-trust context-trust-${candidate.trust}`}>{trustLabel(candidate.trust)}</span>
              <span>{percent(candidate.freshness_score)}</span>
              <span className="context-token-cell">
                {(selection.state === "included" ? selection.included_tokens : candidate.candidate_tokens).toLocaleString()}
              </span>
            </summary>

            <div className="context-decision-detail">
              <div>
                <span>Relevance</span>
                <strong>{percent(candidate.relevance_score)}</strong>
              </div>
              <div>
                <span>Freshness</span>
                <strong>{percent(candidate.freshness_score)}</strong>
              </div>
              <div>
                <span>Required</span>
                <strong>{candidate.required ? "yes" : "no"}</strong>
              </div>
              <div>
                <span>Observed</span>
                <strong>{candidate.provenance.observed_at ? new Date(candidate.provenance.observed_at).toLocaleString() : "not evaluated"}</strong>
              </div>
              <div>
                <span>Fingerprint</span>
                <code title={candidate.provenance.fingerprint}>{shortFingerprint(candidate.provenance.fingerprint)}</code>
              </div>
              <div>
                <span>Candidate cost</span>
                <strong>{candidate.candidate_tokens.toLocaleString()} tokens</strong>
              </div>
            </div>
          </details>
        ))}
      </div>

      <details className="context-evidence-secondary">
        <summary>Repository file evidence {manifest ? `· ${manifest.included_files} included / ${manifest.excluded_files} excluded` : ""}</summary>
        {!manifest ? (
          <p className="muted">No file manifest is available for this build.</p>
        ) : manifest.entries.length === 0 ? (
          <p className="muted">No explicit repository paths were eligible from the Work Item scope.</p>
        ) : (
          <div className="context-file-evidence-list">
            {manifest.entries.map((entry) => (
              <div key={entry.path}>
                <span className={entry.status}>{entry.status}</span>
                <code>{entry.path}</code>
                <small>{exclusionLabel(entry)}</small>
              </div>
            ))}
          </div>
        )}
      </details>

      {coverage && coverage.changed_files_missing_from_context.length > 0 ? (
        <p className="context-evidence-warning">
          Changed outside prepared context: {coverage.changed_files_missing_from_context.join(", ")}
        </p>
      ) : null}
    </section>
  );
}
