import type { ReactNode } from "react";
import type { SemanticTone } from "./semantic";
import { StatusBadge } from "./StatusBadge";

export type EvidenceStateProps = {
  label: string;
  state: string;
  tone: SemanticTone;
  detail?: string;
  children?: ReactNode;
  role?: "status" | "alert";
};

export function EvidenceState({ label, state, tone, detail, children, role }: EvidenceStateProps) {
  return (
    <div className="semantic-evidence" data-semantic-tone={tone} role={role}>
      <span className="semantic-evidence__label">{label}</span>
      <div className="semantic-evidence__state">
        <StatusBadge label={state} tone={tone} />
      </div>
      {detail ? <small className="semantic-evidence__detail">{detail}</small> : null}
      {children ? <div className="semantic-evidence__technical">{children}</div> : null}
    </div>
  );
}
