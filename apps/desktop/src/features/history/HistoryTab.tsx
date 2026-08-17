import { lazy, Suspense, useState } from "react";
import { LoadingState, PanelHeader } from "../../shared/ui/primitives";
import { RunsWorkspace } from "./RunsWorkspace";
import "../../shared/ui/secondary-subnav.css";
import "./history-route.css";

const OutcomesTab = lazy(() => import("../outcomes/OutcomesTab").then((m) => ({ default: m.OutcomesTab })));
const AuditTab = lazy(() => import("../audit/AuditTab").then((m) => ({ default: m.AuditTab })));

type RunsView = "runs" | "outcomes" | "audit";

const VIEWS: { id: RunsView; label: string }[] = [
  { id: "runs", label: "Run evidence" },
  { id: "outcomes", label: "Provider outcomes" },
  { id: "audit", label: "Raw audit" },
];

export function HistoryTab() {
  const [view, setView] = useState<RunsView>("runs");
  return (
    <div className="subnav-host history-tab">
      <div className="changes-summary">
        <PanelHeader
          eyebrow="Runs"
          title="Execution history with inspectable engineering evidence"
          description="Immutable run evidence, provider outcomes, and the raw audit trail."
        />
      </div>
      <div className="subnav" role="tablist" aria-label="Runs views">
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
      <Suspense fallback={<LoadingState message="Loading Runs view…" />}>
        {view === "runs" ? <RunsWorkspace /> : view === "outcomes" ? <OutcomesTab /> : <AuditTab />}
      </Suspense>
    </div>
  );
}
