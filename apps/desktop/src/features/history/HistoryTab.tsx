import { lazy, Suspense, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { WORK_ENGINEERING_SNAPSHOT_KEY, workEngineeringSnapshot } from "../../shared/api/engineering";
import { getDecisionReceipt } from "../../shared/api/verification";
import { LoadingState, Metric, PanelHeader } from "../../shared/ui/primitives";
import { RunsWorkspace } from "./RunsWorkspace";
import "../../shared/ui/secondary-subnav.css";
import "./history-route.css";

const OutcomesTab = lazy(() => import("../outcomes/OutcomesTab").then((m) => ({ default: m.OutcomesTab })));
const AuditTab = lazy(() => import("../audit/AuditTab").then((m) => ({ default: m.AuditTab })));

type RunsView = "runs" | "outcomes" | "audit";

const VIEWS: { id: RunsView; label: string }[] = [
  { id: "runs", label: "Run timeline" },
  { id: "outcomes", label: "Change economics" },
  { id: "audit", label: "Evidence archive" },
];

function DecisionLedgerSummary() {
  const snapshot = useQuery({
    queryKey: WORK_ENGINEERING_SNAPSHOT_KEY,
    queryFn: workEngineeringSnapshot,
    staleTime: 3_000,
    refetchOnWindowFocus: true,
  });
  const receipt = useQuery({
    queryKey: ["verification", "decision-receipt", "latest"],
    queryFn: () => getDecisionReceipt(),
    staleTime: 3_000,
    refetchOnWindowFocus: true,
  });
  const decisions = snapshot.data?.intelligence?.decisions;
  const latestReceipt = receipt.data;

  return (
    <section className="runs-decision-summary" aria-label="Decision receipts summary">
      <div>
        <span className="eyebrow">Decision receipts</span>
        <strong>Why this work ran, waited, or stopped</strong>
        <p>
          {latestReceipt
            ? `Latest: ${latestReceipt.decision_kind.replace(/_/g, " ")} on ${latestReceipt.tree_identity}.`
            : "Receipts keep verification choices bound to a Work Item, tree identity, policy, and evidence state."}
        </p>
      </div>
      <div className="runs-decision-metrics">
        <Metric label="Decisions" value={decisions ? String(decisions.decision_count) : "Not measured"} />
        <Metric label="Verification debt" value={decisions ? String(decisions.verification_debt_count) : "Not measured"} />
        <Metric label="Overrides" value={decisions ? String(decisions.override_count) : "Not measured"} />
      </div>
    </section>
  );
}

export function HistoryTab() {
  const [view, setView] = useState<RunsView>("runs");
  return (
    <div className="subnav-host history-tab">
      <div className="changes-summary">
        <PanelHeader
          eyebrow="Runs"
          title="Execution history with decision evidence"
          description="Inspect the run timeline, the economics of accepted change, and the verified evidence archive."
        />
      </div>
      <DecisionLedgerSummary />
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
