import { useQuery } from "@tanstack/react-query";
import { requestCodeWorkspaceOpen } from "../../shared/api/codeWorkspace";
import {
  REPOSITORY_INTELLIGENCE_KEY,
  repositoryIntelligenceSnapshot,
  type RepositoryContextCandidate,
} from "../../shared/api/repositoryIntelligence";
import { errorToMessage } from "../../shared/utils/helpers";
import "./repository-intelligence.css";

function fileLabel(path: string): string {
  return path.split("/").pop() || path;
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
          <span>{snapshot.data.rust_files_indexed.toLocaleString()} Rust AST indexed</span>
          {snapshot.data.truncated ? <span className="warn">bounded index</span> : null}
        </div>
      ) : null}

      {focus ? (
        <div className="repo-intel-sections">
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
