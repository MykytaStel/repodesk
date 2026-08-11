import { useQuery } from "@tanstack/react-query";
import { requestCodeWorkspaceOpen } from "../../shared/api/codeWorkspace";
import {
  REPOSITORY_INTELLIGENCE_KEY,
  repositoryIntelligenceSnapshot,
  type RepositoryContextCandidate,
  type RepositoryEvidenceLevel,
  type RepositoryLanguageCoverage,
  type RepositorySemanticStrategy,
} from "../../shared/api/repositoryIntelligence";
import { errorToMessage } from "../../shared/utils/helpers";
import "./repository-intelligence.css";

function fileLabel(path: string): string {
  return path.split("/").pop() || path;
}

function evidenceLabel(level: RepositoryEvidenceLevel): string {
  switch (level) {
    case "strong": return "Strong";
    case "bounded": return "Bounded";
    case "unavailable": return "Unavailable";
  }
}

function strategyLabel(strategy: RepositorySemanticStrategy): string {
  switch (strategy) {
    case "rust_ast": return "Rust AST";
    case "script_literal_imports": return "Local literal imports";
    case "unavailable": return "No dependency indexer";
  }
}

function relevantCoverage(
  languages: RepositoryLanguageCoverage[],
  focusLanguage: string | null,
): RepositoryLanguageCoverage[] {
  return languages.filter((item) => (
    item.strategy !== "unavailable" || item.language === focusLanguage
  ));
}

function CandidateRow({ candidate }: { candidate: RepositoryContextCandidate }) {
  return (
    <button
      type="button"
      className="repo-intel-row"
      onClick={() => requestCodeWorkspaceOpen(candidate.path)}
      title={candidate.reasons.join(" · ")}
    >
      <span>
        <strong>{fileLabel(candidate.path)}</strong>
        <code>{candidate.path}</code>
      </span>
      <small>{candidate.score}</small>
    </button>
  );
}

export function RepositoryIntelligenceDrawer({
  projectName,
  path,
  onClose,
}: {
  projectName: string | null | undefined;
  path: string;
  onClose: () => void;
}) {
  const snapshot = useQuery({
    queryKey: [...REPOSITORY_INTELLIGENCE_KEY, projectName ?? "none", path],
    queryFn: () => repositoryIntelligenceSnapshot(path),
    staleTime: 15_000,
    refetchOnWindowFocus: false,
  });

  const focus = snapshot.data?.focus ?? null;
  const coverageLanguages = snapshot.data
    ? relevantCoverage(snapshot.data.coverage.languages, focus?.language ?? null)
    : [];

  return (
    <aside className="repo-intel-drawer" aria-label="Repository intelligence">
      <header>
        <div>
          <strong>Repository intelligence</strong>
          <code>{path}</code>
        </div>
        <button type="button" onClick={onClose} aria-label="Close repository intelligence">×</button>
      </header>

      {snapshot.isLoading ? <div className="focus-empty compact">Building bounded neighborhood…</div> : null}
      {snapshot.isError ? <div className="notice danger">{errorToMessage(snapshot.error)}</div> : null}

      {snapshot.data ? (
        <div className="repo-intel-meta">
          <span>{snapshot.data.indexed_files.toLocaleString()} files visible</span>
          <span>
            {snapshot.data.coverage.semantic_files_indexed.toLocaleString()}/
            {snapshot.data.coverage.semantic_files_eligible.toLocaleString()} semantic files indexed
          </span>
          {snapshot.data.truncated ? <span className="warn">bounded index</span> : null}
        </div>
      ) : null}

      {focus && snapshot.data ? (
        <div className="repo-intel-sections">
          <section className="repo-intel-evidence">
            <div className="repo-intel-evidence-head">
              <h4>Graph evidence</h4>
              <span className={`repo-intel-evidence-badge ${focus.graph_evidence.level}`}>
                {evidenceLabel(focus.graph_evidence.level)}
              </span>
            </div>
            <code className="repo-intel-strategy">{strategyLabel(focus.graph_evidence.strategy)}</code>
            {focus.graph_evidence.reasons.map((reason) => <p key={reason}>{reason}</p>)}
            {focus.graph_evidence.limitations.length > 0 ? (
              <ul className="repo-intel-limitations">
                {focus.graph_evidence.limitations.map((limitation) => <li key={limitation}>{limitation}</li>)}
              </ul>
            ) : null}
          </section>

          <section className="repo-intel-coverage">
            <h4>
              Semantic coverage
              <span>
                {snapshot.data.coverage.semantic_files_indexed}/
                {snapshot.data.coverage.semantic_files_eligible}
              </span>
            </h4>
            <p>Only files with an implemented dependency strategy count as semantic-eligible.</p>
            {coverageLanguages.length === 0 ? <p>No semantic dependency indexer is active.</p> : coverageLanguages.map((item) => (
              <div className="repo-intel-coverage-row" key={item.language} title={item.limitations.join(" · ")}>
                <span>
                  <strong>{item.language}</strong>
                  <code>{strategyLabel(item.strategy)}</code>
                </span>
                <span>
                  <i className={`repo-intel-evidence-badge ${item.evidence_level}`}>
                    {evidenceLabel(item.evidence_level)}
                  </i>
                  <small>
                    {item.semantic_files_indexed}/{item.visible_files}
                    {item.truncated ? " · index capped" : ""}
                  </small>
                </span>
              </div>
            ))}
          </section>

          <section>
            <h4>Dependencies <span>{focus.dependencies.length}</span></h4>
            {focus.dependencies.length === 0 ? <p>No resolved local dependencies.</p> : focus.dependencies.map((item) => (
              <button type="button" className="repo-intel-row" key={`dep:${item.path}`} onClick={() => requestCodeWorkspaceOpen(item.path)} title={item.reason}>
                <span><strong>{fileLabel(item.path)}</strong><code>{item.path}</code></span>
              </button>
            ))}
          </section>

          <section>
            <h4>Dependents <span>{focus.dependents.length}</span></h4>
            {focus.dependents.length === 0 ? <p>No resolved local dependents.</p> : focus.dependents.map((item) => (
              <button type="button" className="repo-intel-row" key={`dependent:${item.path}`} onClick={() => requestCodeWorkspaceOpen(item.path)} title={item.reason}>
                <span><strong>{fileLabel(item.path)}</strong><code>{item.path}</code></span>
              </button>
            ))}
          </section>

          <section>
            <h4>Closest tests <span>{focus.closest_tests.length}</span></h4>
            {focus.closest_tests.length === 0 ? <p>No deterministic test candidate found.</p> : focus.closest_tests.map((item) => (
              <button type="button" className="repo-intel-row" key={`test:${item.path}`} onClick={() => requestCodeWorkspaceOpen(item.path)} title={item.reason}>
                <span><strong>{fileLabel(item.path)}</strong><code>{item.reason}</code></span>
                <small>{item.score}</small>
              </button>
            ))}
          </section>

          <section>
            <h4>Co-change history <span>{focus.co_changes.length}</span></h4>
            {focus.co_changes.length === 0 ? <p>No bounded Git co-change evidence.</p> : focus.co_changes.map((item) => (
              <button type="button" className="repo-intel-row" key={`co:${item.path}`} onClick={() => requestCodeWorkspaceOpen(item.path)}>
                <span><strong>{fileLabel(item.path)}</strong><code>{item.path}</code></span>
                <small>{item.commits_together}/{item.focus_commits_sampled}</small>
              </button>
            ))}
          </section>

          <section className="repo-intel-context">
            <h4>Context candidates <span>{focus.context_candidates.length}</span></h4>
            <p>Explainable candidates only. This does not mutate the Work Item context.</p>
            {focus.context_candidates.map((candidate) => <CandidateRow key={`context:${candidate.path}`} candidate={candidate} />)}
          </section>
        </div>
      ) : snapshot.data && !snapshot.isLoading ? (
        <div className="focus-empty compact">No focused repository intelligence for this file.</div>
      ) : null}
    </aside>
  );
}
