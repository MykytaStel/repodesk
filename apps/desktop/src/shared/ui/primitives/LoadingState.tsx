import type { StateScope } from "./EmptyState";

export function LoadingState({ message, scope = "inline" }: { message: string; scope?: StateScope }) {
  return (
    <div className={`semantic-state semantic-state--${scope} semantic-state--loading`} role="status" aria-live="polite">
      <span className="semantic-state__indicator" aria-hidden="true" />
      <span>{message}</span>
    </div>
  );
}
