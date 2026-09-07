import type { OrchestrationRun, PhaseProgress } from "../../shared/api/orchestrate";

type RunControlCardProps = {
  phase: PhaseProgress | null;
  latestRun: OrchestrationRun | null;
  onOpenRuns: () => void;
};

function unknown(value: string | number | null | undefined): string {
  return value == null || value === "" ? "Unknown" : String(value);
}

export function RunControlCard({ phase, latestRun, onOpenRuns }: RunControlCardProps) {
  const totalTokens = latestRun
    ? latestRun.total_input_tokens + latestRun.total_output_tokens
    : null;

  return (
    <section className="work-control-card run-control-card" aria-label="Run Control">
      <div className="work-control-card-heading">
        <div>
          <span className="eyebrow">Run Control</span>
          <h2>Keep execution bounded</h2>
        </div>
        <span className="work-control-state is-neutral">{phase?.current ?? "Unknown"}</span>
      </div>
      <div className="work-control-facts" role="list" aria-label="Run facts">
        <div role="listitem"><span>Latest run</span><strong>{unknown(latestRun?.status)}</strong></div>
        <div role="listitem"><span>Tokens</span><strong>{totalTokens == null ? "Not measured" : totalTokens.toLocaleString()}</strong></div>
        <div role="listitem"><span>Spend</span><strong>{latestRun == null ? "Not measured" : `${latestRun.total_cost_units.toFixed(3)} units`}</strong></div>
        <div role="listitem"><span>Attempts</span><strong>{latestRun == null ? "Unknown" : "1 recorded run"}</strong></div>
      </div>
      <p className="work-control-note">Execution remains owned by the existing workflow. This surface records decisions and evidence; it does not silently start a check.</p>
      <button type="button" className="link-cta work-control-link" onClick={onOpenRuns}>Open run timeline</button>
    </section>
  );
}
