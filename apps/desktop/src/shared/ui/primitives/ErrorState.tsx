import type { ReactNode } from "react";
import type { StateScope } from "./EmptyState";

export function ErrorState({
  title,
  detail,
  scope = "inline",
  action,
}: {
  title: string;
  detail?: ReactNode;
  scope?: StateScope;
  action?: ReactNode;
}) {
  return (
    <div
      className={`semantic-state semantic-state--${scope} semantic-state--error`}
      role="alert"
      data-semantic-tone="critical"
    >
      <strong>{title}</strong>
      {detail ? <span>{detail}</span> : null}
      {action ? <div className="semantic-state__action">{action}</div> : null}
    </div>
  );
}
