import { FileFindings, RepoPilotTrendPoint, severityTone } from "../../shared/api/repopilot";

/** Tiny inline SVG sparkline of recent health scores (0–100). */
export function HealthTrend({ points }: { points: RepoPilotTrendPoint[] }) {
  const scored = points.filter((p) => p.health_score != null) as Required<RepoPilotTrendPoint>[];
  if (scored.length < 2) return null;
  const w = 160;
  const h = 36;
  const xs = scored.map((_, i) => (i / (scored.length - 1)) * w);
  const ys = scored.map((p) => h - (Math.max(0, Math.min(100, p.health_score!)) / 100) * h);
  const path = xs.map((x, i) => `${i === 0 ? "M" : "L"}${x.toFixed(1)},${ys[i].toFixed(1)}`).join(" ");
  const latest = scored[scored.length - 1].health_score!;
  const previous = scored[scored.length - 2].health_score!;
  const delta = latest - previous;
  const tone = delta > 0 ? "ok" : delta < 0 ? "danger" : "neutral";
  return (
    <div className="health-trend">
      <svg width={w} height={h} viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" role="img" aria-label="Health score trend">
        <polyline points={`0,${h} ${xs.map((x, i) => `${x},${ys[i]}`).join(" ")} ${w},${h}`} className="trend-fill" />
        <path d={path} className="trend-line" fill="none" />
      </svg>
      <div className="trend-meta">
        <strong>Health {latest}</strong>
        <span className={`pill ${tone}`}>{delta > 0 ? `+${delta}` : delta}</span>
        <small>{scored.length} reviews</small>
      </div>
    </div>
  );
}

export function FindingRow({ finding }: { finding: FileFindings["findings"][number] }) {
  return (
    <li className="finding">
      <span className={`pill ${severityTone(finding.severity)}`}>{finding.severity}</span>
      <strong>{finding.title}</strong>
      {finding.line != null && <small>line {finding.line}</small>}
      {finding.rule && <small className="muted">{finding.rule}</small>}
    </li>
  );
}
