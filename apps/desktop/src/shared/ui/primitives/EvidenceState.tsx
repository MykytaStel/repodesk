import type { ReactNode } from "react";
import type { SemanticTone } from "./semantic";
import { StatusBadge } from "./StatusBadge";

export type EvidenceStateProps = {
  label: string;
  state: string;
  tone: SemanticTone;
  detail?: string;
  children?: ReactNode;
};

export function EvidenceState({ label, state, tone, detail, children }: EvidenceStateProps) {
  return (
    <div className="semantic-evidence" data-semantic-tone={tone}>
      <span className="semantic-evidence__label">{label}</span>
      <div className="semantic-evidence__state">
        <StatusBadge label={state} tone={tone} />
      </div>
      {detail ? <small className="semantic-evidence__detail">{detail}</small> : null}
      {children ? <div className="semantic-evidence__technical">{children}</div> : null}
    </div>
  );
}
