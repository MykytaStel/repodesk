import { FormEvent, useState } from "react";
import {
  searchCodeWorkspaceProject,
  type CodeProjectSearchMatch,
  type CodeProjectSearchResult,
} from "../../shared/api/codeWorkspace";
import { errorToMessage } from "../../shared/utils/helpers";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

export function CodeProjectSearch({
  onClose,
  onOpen,
}: {
  onClose: () => void;
  onOpen: (match: CodeProjectSearchMatch) => void;
}) {
  const [query, setQuery] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [result, setResult] = useState<CodeProjectSearchResult | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const literal = query.trim();
    if (!literal || pending) return;

    setPending(true);
    setError(null);
    try {
      const next = await searchCodeWorkspaceProject({
        query: literal,
        case_sensitive: caseSensitive,
        limit: 200,
      });
      setResult(next);
    } catch (cause) {
      setResult(null);
      setError(errorToMessage(cause));
    } finally {
      setPending(false);
    }
  };

  return (
    <aside className="code-explorer code-project-search" aria-label="Project search">
      <div className="code-explorer-head">
        <strong>Search</strong>
        <button type="button" className="code-search-close" onClick={onClose} aria-label="Close project search">
          ×
        </button>
      </div>

      <form className="code-project-search-form" onSubmit={(event) => void submit(event)}>
        <div className="code-project-search-input-row">
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search project text…"
            aria-label="Search project text"
            spellCheck={false}
            autoFocus
          />
          <button
            type="button"
            className={`code-search-case${caseSensitive ? " active" : ""}`}
            aria-pressed={caseSensitive}
            title="Match case"
            onClick={() => setCaseSensitive((current) => !current)}
          >
            Aa
          </button>
        </div>
        <button type="submit" className="tiny-button" disabled={!query.trim() || pending}>
          {pending ? "Searching…" : "Search"}
        </button>
      </form>

      {error ? <div className="code-search-notice danger">{error}</div> : null}

      <div className="code-search-results" role="list" aria-label="Project search results">
        {!result && !pending && !error ? (
          <div className="code-search-empty">
            <strong>Search repository text.</strong>
            <span>Literal search only · UTF-8 text · guarded Code Workspace scope.</span>
          </div>
        ) : null}
        {result && result.matches.length === 0 ? (
          <div className="code-search-empty">
            <strong>No matches.</strong>
            <span>Try another literal or change case matching.</span>
          </div>
        ) : null}
        {result?.matches.map((match, index) => (
          <button
            type="button"
            className="code-search-result"
            role="listitem"
            key={`${match.path}:${match.line}:${match.column}:${index}`}
            onClick={() => onOpen(match)}
          >
            <span className="code-search-result-location">
              <strong>{match.path}</strong>
              <code>{match.line}:{match.column}</code>
            </span>
            <code className="code-search-preview">{match.preview || " "}</code>
          </button>
        ))}
      </div>

      {result ? (
        <div className={`code-search-summary${result.truncated ? " warn" : ""}`}>
          <span>
            {result.matches.length} match{result.matches.length === 1 ? "" : "es"}
            {` · ${result.scanned_files.toLocaleString()} files`}
            {` · ${formatBytes(result.scanned_bytes)}`}
          </span>
          {result.skipped_files > 0 ? <span>{result.skipped_files} files skipped by text/safety policy</span> : null}
          {result.truncated ? (
            <strong>
              {result.workspace_truncated ? "Repository index or search budget was capped. Refine the query." : "Search results were capped. Refine the query."}
            </strong>
          ) : null}
        </div>
      ) : null}
    </aside>
  );
}
