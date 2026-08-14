import { lazy, Suspense, useState } from "react";
import type { TabId } from "../../shared/types/api";
import "../../shared/ui/secondary-subnav.css";
import "./models-cost-route.css";

// "Models & Cost" merges the two runtime-economy surfaces: which providers/models
// are reachable (health + discovery) and what the work is costing (usage ledger +
// rate card). One surface so the cost of a route sits next to its availability.
const ModelsTab = lazy(() => import("../models/ModelsTab").then((m) => ({ default: m.ModelsTab })));
const TokensTab = lazy(() => import("../tokens/TokensTab").then((m) => ({ default: m.TokensTab })));

type ModelsCostView = "models" | "cost";

const VIEWS: { id: ModelsCostView; label: string }[] = [
  { id: "models", label: "Runtime health" },
  { id: "cost", label: "Usage & cost" },
];

export function ModelsCostTab({ setActiveTab }: { setActiveTab: (tab: TabId) => void }) {
  const [view, setView] = useState<ModelsCostView>("models");
  return (
    <div className="subnav-host models-cost-tab">
      <div className="changes-summary">
        <div>
          <p className="eyebrow">Models &amp; Cost</p>
          <strong>Which models are reachable &amp; what the work costs</strong>
        </div>
      </div>
      <div className="subnav" role="tablist" aria-label="Models and cost views">
        {VIEWS.map((item) => (
          <button
            key={item.id}
            role="tab"
            aria-selected={view === item.id}
            className={view === item.id ? "selected" : ""}
            onClick={() => setView(item.id)}
          >
            {item.label}
          </button>
        ))}
      </div>
      <Suspense fallback={<p className="muted">Loading…</p>}>
        {view === "models" ? <ModelsTab setActiveTab={setActiveTab} /> : <TokensTab />}
      </Suspense>
    </div>
  );
}
