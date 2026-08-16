import type { ReactNode } from "react";

export function ActionBar({
  primary,
  secondary,
  destructive,
  detail,
}: {
  primary?: ReactNode;
  secondary?: ReactNode;
  destructive?: ReactNode;
  detail?: ReactNode;
}) {
  return (
    <div className="semantic-action-bar">
      <div className="semantic-action-bar__actions">
        {primary ? <div className="semantic-action-bar__primary">{primary}</div> : null}
        {secondary ? <div className="semantic-action-bar__secondary">{secondary}</div> : null}
        {destructive ? <div className="semantic-action-bar__destructive">{destructive}</div> : null}
      </div>
      {detail ? <div className="semantic-action-bar__detail">{detail}</div> : null}
    </div>
  );
}
