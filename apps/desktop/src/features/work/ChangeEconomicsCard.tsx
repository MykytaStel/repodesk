import type { OrchestrationRun } from "../../shared/api/orchestrate";
import type { AiUsageReport } from "../../shared/api/observability";

type ChangeEconomicsCardProps = {
  latestRun: OrchestrationRun | null;
  report: AiUsageReport | null;
};

function measured(value: number | null | undefined, suffix = "") {
  return value == null ? "Not measured" : `${value.toFixed(3)}${suffix}`;
}

export function ChangeEconomicsCard({ latestRun, report }: ChangeEconomicsCardProps) {
  const acceptedCost = report?.accepted_change_cost_units ?? null;
  const acceptedCount = report?.accepted_change_count ?? 0;
  const costPerAccepted = acceptedCost != null && acceptedCount > 0 ? acceptedCost / acceptedCount : null;

  return (
    <section className="work-control-card change-economics-card" aria-label="Change Economics">
      <div className="work-control-card-heading">
        <div>
          <span className="eyebrow">Change Economics</span>
          <h2>Spend that led to acceptance</h2>
        </div>
        <span className="work-control-state is-unknown">Evidence-led</span>
      </div>
      <div className="work-control-facts" role="list" aria-label="Change economics facts">
        <div role="listitem"><span>Cost / accepted change</span><strong>{measured(costPerAccepted, " units")}</strong></div>
        <div role="listitem"><span>Correction cost</span><strong>{measured(report?.correction_cost_units, " units")}</strong></div>
        <div role="listitem"><span>Total recorded spend</span><strong>{latestRun == null ? "Not measured" : `${latestRun.total_cost_units.toFixed(3)} units`}</strong></div>
        <div role="listitem"><span>Evidence timestamp</span><strong>{latestRun?.finished_at ?? "Unknown"}</strong></div>
      </div>
      <p className="work-control-note">No productivity score: only cost, accepted changes, retries, and the evidence that supports them.</p>
    </section>
  );
}
