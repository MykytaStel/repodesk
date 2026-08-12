import { useState } from "react";
import type { TabId } from "../../shared/types/api";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { ContextInspectorCard } from "./ContextInspectorCard";
import { WorkItemContractCard } from "./WorkItemContractCard";
import { WorkTab } from "./WorkTab";

export function WorkSurface({ setActiveTab }: { setActiveTab: (tab: TabId) => void }) {
  const { projectName, taskTitle, hasTask } = useWorkspace();
  const [contractOpen, setContractOpen] = useState(false);
  const [contextOpen, setContextOpen] = useState(false);

  const toggleContract = () => {
    setContextOpen(false);
    setContractOpen((open) => !open);
  };

  const toggleContext = () => {
    setContractOpen(false);
    setContextOpen((open) => !open);
  };

  return (
    <div className="work-surface-v2 focus-work-surface">
      <header className="work-cockpit-bar" aria-label="Active Work Item context">
        <div className="work-cockpit-identity">
          <span className="eyebrow">Active work</span>
          <strong>{taskTitle || "No Work Item selected"}</strong>
          <small>{projectName ? `Project · ${projectName}` : "Connect a project to begin"}</small>
        </div>

        <nav className="work-cockpit-actions" aria-label="Work Item tools">
          <button
            type="button"
            className={contractOpen ? "active" : ""}
            onClick={toggleContract}
            disabled={!hasTask}
            aria-pressed={contractOpen}
          >
            Contract
          </button>
          <button
            type="button"
            className={contextOpen ? "active" : ""}
            onClick={toggleContext}
            disabled={!hasTask}
            aria-pressed={contextOpen}
          >
            Context evidence
          </button>
          <button type="button" onClick={() => setActiveTab("memory")} disabled={!projectName}>
            Knowledge
          </button>
          <button type="button" onClick={() => setActiveTab("changes")} disabled={!projectName}>
            Changes
          </button>
        </nav>
      </header>

      {contractOpen ? <WorkItemContractCard /> : null}
      {contextOpen ? <ContextInspectorCard /> : null}
      <WorkTab setActiveTab={setActiveTab} />
    </div>
  );
}
