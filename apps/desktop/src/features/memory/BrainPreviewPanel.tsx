import { formatNumber } from "../../shared/utils/helpers";
import type { BrainPreview } from "../../shared/api/memory";

export function BrainPreviewPanel({
  preview,
  loading,
}: {
  preview: BrainPreview | null;
  loading: boolean;
}) {
  return (
    <section className="panel wide-panel">
      <div className="panel-title-row">
        <div>
          <p className="eyebrow">What the AI sees</p>
          <h2>Injected memory slice</h2>
        </div>
        {preview && (
          <div className="row-meta">
            <span className="pill">{formatNumber(preview.estimated_tokens)} tokens</span>
            <span className="muted" style={{ fontSize: 12 }}>
              {preview.included}/{preview.total_active} included
              {preview.excluded > 0 ? ` - ${preview.excluded} dropped (budget)` : ""}
            </span>
          </div>
        )}
      </div>
      <pre className="scroll-area" style={{ maxHeight: 220, whiteSpace: "pre-wrap", fontSize: 13 }}>
        {preview?.markdown ?? (loading ? "Loading..." : "No active memory yet.")}
      </pre>
      <p className="muted" style={{ fontSize: 12 }}>
        This exact slice is ranked (pinned, task relevance, recency) and injected into
        context.md and the smart pack for every agent.
      </p>
    </section>
  );
}
