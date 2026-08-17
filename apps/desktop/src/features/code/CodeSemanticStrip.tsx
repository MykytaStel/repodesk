import { StatusBadge } from "../../shared/ui/primitives";
import {
  codeOriginSemantic,
  codeReviewSemantic,
  codeScopeSemantic,
  codeVerificationSemantic,
} from "./codeSemantic";
import type { SemanticFileState } from "./useSemanticCodeState";

function BadgeItem({
  label,
  tone,
  ariaLabel,
}: {
  label: string;
  tone: Parameters<typeof StatusBadge>[0]["tone"];
  ariaLabel?: string;
}) {
  return (
    <span className="code-semantic-item">
      <StatusBadge label={label} tone={tone} ariaLabel={ariaLabel} />
    </span>
  );
}

export function CodeSemanticStrip({
  semantic,
  dirty,
}: {
  semantic: SemanticFileState;
  dirty: boolean;
}) {
  const visible = Boolean(
    semantic.workItemId
    || semantic.scopeState
    || semantic.reviewState
    || semantic.verificationState
    || semantic.origin !== "unknown"
    || semantic.errors
    || semantic.warnings
    || semantic.gitLines.length
    || dirty,
  );
  if (!visible) return null;

  const scope = semantic.scopeState ? codeScopeSemantic(semantic.scopeState) : null;
  const review = semantic.reviewState ? codeReviewSemantic(semantic.reviewState) : null;
  const verification = semantic.verificationState
    ? codeVerificationSemantic(semantic.verificationState, dirty)
    : null;
  const origin = semantic.origin !== "unknown"
    ? codeOriginSemantic(semantic.origin, semantic.originLabel)
    : null;

  return (
    <div className={`semantic-code-strip scope-${semantic.scopeState ?? "none"}`} aria-label="Code engineering state">
      {semantic.workItemId ? (
        <span className="code-semantic-item">
          <span title="Current Work Item"><strong>Work</strong> {semantic.workItemId}</span>
        </span>
      ) : null}
      {scope ? <BadgeItem label={scope.label} tone={scope.tone} ariaLabel={`Scope: ${scope.label}`} /> : null}
      {review ? <BadgeItem label={review.label} tone={review.tone} ariaLabel={`Review: ${review.label}`} /> : null}
      {verification ? (
        <BadgeItem
          label={verification.label}
          tone={verification.tone}
          ariaLabel={`Verification: ${verification.label}`}
        />
      ) : null}
      {origin ? (
        <BadgeItem
          label={origin.label}
          tone={origin.tone}
          ariaLabel={`ChangeSet origin: ${origin.detail ?? origin.label}`}
        />
      ) : null}
      {semantic.errors > 0 ? (
        <BadgeItem
          label={`${semantic.errors} error${semantic.errors === 1 ? "" : "s"}`}
          tone="critical"
        />
      ) : null}
      {semantic.warnings > 0 ? (
        <BadgeItem
          label={`${semantic.warnings} warning${semantic.warnings === 1 ? "" : "s"}`}
          tone="attention"
        />
      ) : null}
      {semantic.gitLines.length > 0 && !dirty ? (
        <BadgeItem
          label={`${semantic.gitLines.length} Git line${semantic.gitLines.length === 1 ? "" : "s"}`}
          tone="info"
        />
      ) : null}
    </div>
  );
}
