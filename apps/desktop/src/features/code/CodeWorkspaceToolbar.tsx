import type { SemanticState } from "../../shared/ui/primitives";
import { StatusBadge } from "../../shared/ui/primitives";
import { IdeIcon } from "./IdeIcon";

export function CodeWorkspaceToolbar({
  project,
  fileCount,
  indexSemantic,
  dirtyCount,
  dirtySemantic,
  canShowRepositoryContext,
  repositoryContextOpen,
  reviewPending,
  reviewTotal,
  insightsOpen,
  onToggleRepositoryContext,
  onAnalyze,
  onToggleInsights,
  onReviewChanges,
  reviewFile,
}: {
  project: string;
  fileCount: number;
  indexSemantic: SemanticState;
  dirtyCount: number;
  dirtySemantic: SemanticState | null;
  canShowRepositoryContext: boolean;
  repositoryContextOpen: boolean;
  reviewPending: boolean;
  reviewTotal: number | null;
  insightsOpen: boolean;
  onToggleRepositoryContext: () => void;
  onAnalyze: () => void;
  onToggleInsights: () => void;
  onReviewChanges: () => void;
  reviewFile: boolean;
}) {
  return (
    <header className="code-workspace-toolbar">
      <div className="code-workspace-title">
        <strong>Code</strong>
        <span>{project}</span>
        <span>{fileCount.toLocaleString()} files</span>
        <StatusBadge
          label={indexSemantic.label}
          tone={indexSemantic.tone}
          ariaLabel={indexSemantic.detail ?? indexSemantic.label}
        />
        {dirtySemantic ? (
          <StatusBadge
            label={`${dirtyCount} unsaved`}
            tone={dirtySemantic.tone}
            ariaLabel={`${dirtyCount} unsaved editor tab${dirtyCount === 1 ? "" : "s"}`}
          />
        ) : null}
      </div>
      <div className="code-workspace-actions ide-icon-toolbar" role="toolbar" aria-label="Code workspace actions">
        {canShowRepositoryContext ? (
          <button
            type="button"
            className={`ide-icon-button${repositoryContextOpen ? " active" : ""}`}
            aria-label="Repository context"
            title="Repository context"
            onClick={onToggleRepositoryContext}
          >
            <IdeIcon name="context" />
          </button>
        ) : null}
        <button
          type="button"
          className="ide-icon-button"
          aria-label={reviewPending ? "Analyzing changes" : "Analyze changes"}
          title={reviewPending ? "Analyzing changes…" : "Analyze changes"}
          disabled={reviewPending}
          onClick={onAnalyze}
        >
          <IdeIcon name="analyze" />
        </button>
        {reviewTotal != null ? (
          <button
            type="button"
            className={`ide-icon-button${insightsOpen ? " active" : ""}`}
            aria-label={`Findings ${reviewTotal}`}
            title={`${reviewTotal} engineering findings`}
            onClick={onToggleInsights}
          >
            <IdeIcon name="more" />
            <span className="ide-icon-count">{reviewTotal > 99 ? "99+" : reviewTotal}</span>
          </button>
        ) : null}
        <button
          type="button"
          className="ide-icon-button"
          aria-label={reviewFile ? "Review file change" : "Review changes"}
          title={reviewFile ? "Review file change" : "Review changes"}
          onClick={onReviewChanges}
        >
          <IdeIcon name="changes" />
        </button>
      </div>
    </header>
  );
}