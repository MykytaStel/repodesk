import { formatNumber } from "../../shared/utils/helpers";
import type { Tone } from "./constants";
import type { MemoryStats } from "./utils";

export function MemoryMetrics({ stats }: { stats: MemoryStats }) {
  return (
    <div className="card-row">
      <BrainMetric label="Active entries" value={formatNumber(stats.activeCount)} detail="in the brain" tone="ok" />
      <BrainMetric label="Pinned" value={formatNumber(stats.pinnedCount)} detail="always in context" tone="neutral" />
      <BrainMetric
        label="Pending proposals"
        value={formatNumber(stats.pendingCount)}
        detail="awaiting review"
        tone={stats.pendingCount > 0 ? "warn" : "neutral"}
      />
      <BrainMetric
        label="Open conflicts"
        value={formatNumber(stats.conflictCount)}
        detail="need resolution"
        tone={stats.conflictCount > 0 ? "danger" : "neutral"}
      />
    </div>
  );
}

function BrainMetric({
  label,
  value,
  detail,
  tone,
}: {
  label: string;
  value: string;
  detail: string;
  tone: Tone;
}) {
  return (
    <section className={`panel metric ${tone}`}>
      <p className="eyebrow">{label}</p>
      <h2>{value}</h2>
      <p className="muted">{detail}</p>
    </section>
  );
}
