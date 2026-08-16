import type { SemanticTone } from "./semantic";

export function Metric({ label, value, detail, tone = "neutral" }: { label: string; value: string; detail?: string; tone?: SemanticTone }) {
  return (
    <div className="semantic-metric" data-semantic-tone={tone}>
      <span>{label}</span>
      <strong>{value}</strong>
      {detail ? <small>{detail}</small> : null}
    </div>
  );
}
