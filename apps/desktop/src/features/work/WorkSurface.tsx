import { useState } from "react";
import type { TabId } from "../../shared/types/api";
import { WorkItemContractCard } from "./WorkItemContractCard";
import { WorkTab } from "./WorkTab";

export function WorkSurface({ setActiveTab }: { setActiveTab: (tab: TabId) => void }) {
  const [contractOpen, setContractOpen] = useState(false);

  return (
    <div className="work-surface-v2 focus-work-surface">
      <div className="work-focus-toolbar" aria-label="Work Item controls">
        <button
          type="button"
          className={`work-focus-tool${contractOpen ? " active" : ""}`}
          onClick={() => setContractOpen((open) => !open)}
        >
          <strong>Scope & acceptance</strong>
          <span>{contractOpen ? "Hide contract" : "Open Engineering Contract"}</span>
        </button>
        <button type="button" className="work-focus-tool" onClick={() => setActiveTab("memory")}>
          <strong>Project Knowledge</strong>
          <span>Review reusable engineering knowledge</span>
        </button>
      </div>

      {contractOpen ? <WorkItemContractCard /> : null}
      <WorkTab setActiveTab={setActiveTab} />
    </div>
  );
}
