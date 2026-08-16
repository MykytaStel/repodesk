export type StateScope = "inline" | "surface";

export function EmptyState({ message, hint, scope = "inline" }: { message: string; hint?: string; scope?: StateScope }) {
  return (
    <div className={`semantic-state semantic-state--${scope} semantic-state--empty`}>
      <strong>{message}</strong>
      {hint ? <span>{hint}</span> : null}
    </div>
  );
}
