import type { HTMLAttributes } from "react";
import type { SemanticTone } from "./semantic";

export type StatusBadgeProps = {
  label: string;
  tone: SemanticTone;
  ariaLabel?: string;
} & Omit<HTMLAttributes<HTMLSpanElement>, "children">;

export function StatusBadge({ label, tone, ariaLabel, className = "", ...props }: StatusBadgeProps) {
  return (
    <span
      {...props}
      className={`semantic-status semantic-status--${tone} ${className}`.trim()}
      data-semantic-tone={tone}
      aria-label={ariaLabel}
    >
      {label}
    </span>
  );
}
