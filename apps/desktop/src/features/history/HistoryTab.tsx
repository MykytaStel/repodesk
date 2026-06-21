import { lazy, Suspense, useState } from "react";

// "History" merges what the task produced and what the system learned/recorded:
// orchestration outcomes, the memory brain, and the audit trail.
const OutcomesTab = lazy(() => import("../outcomes/OutcomesTab").then((m) => ({ default: m.OutcomesTab })));
const MemoryTab = lazy(() => import("../memory/MemoryTab").then((m) => ({ default: m.MemoryTab })));
const AuditTab = lazy(() => import("../audit/AuditTab").then((m) => ({ default: m.AuditTab })));

type HistoryView = "outcomes" | "memory" | "audit";

const VIEWS: { id: HistoryView; label: string }[] = [
  { id: "outcomes", label: "Outcomes & runs" },
  { id: "memory", label: "Memory" },
  { id: "audit", label: "Audit trail" },
];

export function HistoryTab() {
  const [view, setView] = useState<HistoryView>("outcomes");
  return (
    <div className="subnav-host">
      <div className="subnav" role="tablist" aria-label="History views">
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
        {view === "outcomes" ? <OutcomesTab /> : view === "memory" ? <MemoryTab /> : <AuditTab />}
      </Suspense>
    </div>
  );
}
