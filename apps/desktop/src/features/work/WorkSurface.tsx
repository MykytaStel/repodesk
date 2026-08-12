import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import type { TabId } from "../../shared/types/api";
import * as orchestrateApi from "../../shared/api/orchestrate";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { ContextInspectorCard } from "./ContextInspectorCard";
import { WorkIntelligenceCard, WorkIntelligenceRailSummary } from "./WorkIntelligenceCard";
import { WorkItemContractCard } from "./WorkItemContractCard";
import { WorkTab } from "./WorkTab";

const PHASE_KEY = ["work", "phase-state"] as const;

type Inspector = "contract" | "context" | "intelligence" | null;

const PHASE_LABELS: Record<orchestrateApi.Phase, string> = {
  scope: "Scope",
  prepare: "Prepare",
  execute: "Execute",
  review: "Review",
  verify: "Verify",
  finish: "Finish",
};

const INSPECTOR_META: Record<Exclude<Inspector, null>, { title: string; hint: string }> = {
  contract: {
    title: "Engineering Contract",
    hint: "Scope, protected paths and acceptance criteria for this Work Item.",
  },
  context: {
    title: "Context Evidence",
    hint: "The exact bounded context decisions RepoDesk can hand to workers.",
  },
  intelligence: {
    title: "AI Usage Intelligence",
    hint: "Explainable context, orchestration and outcome-efficiency signals from Work Item evidence.",
  },
};

export function WorkSurface({ setActiveTab }: { setActiveTab: (tab: TabId) => void }) {
  const { projectName, taskTitle, hasTask } = useWorkspace();
  const [inspector, setInspector] = useState<Inspector>(null);
  const phase = useQuery({
    queryKey: PHASE_KEY,
    queryFn: () => orchestrateApi.workPhaseState(),
    enabled: hasTask,
    staleTime: 1_500,
    refetchOnWindowFocus: true,
  });

  const progress = phase.data ?? null;
  const phaseIndex = progress
    ? Math.max(0, progress.phases.findIndex((item) => item.phase === progress.current))
    : -1;
  const donePhases = progress?.phases.filter((item) => item.status === "done").length ?? 0;
  const phasePercent = progress?.phases.length
    ? Math.round((donePhases / progress.phases.length) * 100)
    : 0;

  const toggleInspector = (next: Exclude<Inspector, null>) => {
    setInspector((current) => (current === next ? null : next));
  };

  const inspectorMeta = inspector ? INSPECTOR_META[inspector] : null;

  return (
    <div className={`work-workbench-v3${inspector ? " inspector-open" : ""}`}>
      <aside className="work-command-rail" aria-label="Active Work Item navigation">
        <section className="work-rail-identity">
          <span className="eyebrow">Work item</span>
          <strong title={taskTitle || undefined}>{taskTitle || "No Work Item selected"}</strong>
          <small>{projectName || "No project connected"}</small>
        </section>

        <section className="work-rail-phase" aria-label="Workflow position">
          <div className="work-rail-phase-line">
            <span>Current phase</span>
            <strong>{progress ? PHASE_LABELS[progress.current] : hasTask ? "Loading…" : "—"}</strong>
          </div>
          <div className="work-rail-progress" aria-hidden="true">
            <span style={{ width: `${phasePercent}%` }} />
          </div>
          <small>
            {progress
              ? progress.complete
                ? "Workflow complete"
                : `Step ${phaseIndex + 1} of ${progress.phases.length}`
              : phase.isError
                ? "Workflow evidence unavailable"
                : "Select work to begin"}
          </small>
        </section>

        {hasTask ? <WorkIntelligenceRailSummary /> : null}

        <nav className="work-rail-section" aria-label="Work Item evidence">
          <span className="work-rail-label">Inspect</span>
          <button
            type="button"
            className={inspector === "contract" ? "active" : ""}
            onClick={() => toggleInspector("contract")}
            disabled={!hasTask}
            aria-pressed={inspector === "contract"}
          >
            <span>Contract</span>
            <small>Scope + acceptance</small>
          </button>
          <button
            type="button"
            className={inspector === "context" ? "active" : ""}
            onClick={() => toggleInspector("context")}
            disabled={!hasTask}
            aria-pressed={inspector === "context"}
          >
            <span>Context</span>
            <small>AI packet evidence</small>
          </button>
          <button
            type="button"
            className={inspector === "intelligence" ? "active" : ""}
            onClick={() => toggleInspector("intelligence")}
            disabled={!hasTask}
            aria-pressed={inspector === "intelligence"}
          >
            <span>Intelligence</span>
            <small>Tokens + agents + outcomes</small>
          </button>
        </nav>

        <nav className="work-rail-section work-rail-related" aria-label="Related work surfaces">
          <span className="work-rail-label">Related</span>
          <button type="button" onClick={() => setActiveTab("code")} disabled={!projectName}>
            <span>Code</span><small>Repository</small>
          </button>
          <button type="button" onClick={() => setActiveTab("changes")} disabled={!projectName}>
            <span>Changes</span><small>Diff + review</small>
          </button>
          <button type="button" onClick={() => setActiveTab("history")} disabled={!hasTask}>
            <span>Runs</span><small>Execution evidence</small>
          </button>
          <button type="button" onClick={() => setActiveTab("memory")} disabled={!projectName}>
            <span>Knowledge</span><small>Project memory</small>
          </button>
        </nav>

        <button
          type="button"
          className="work-rail-orchestrate"
          onClick={() => setActiveTab("orchestrate")}
          disabled={!hasTask}
        >
          Advanced orchestration
        </button>
      </aside>

      <main className="work-primary-pane" aria-label="Current Work Item">
        <WorkTab setActiveTab={setActiveTab} />
      </main>

      {inspector && inspectorMeta ? (
        <aside className="work-inspector-pane" aria-label={inspectorMeta.title}>
          <header className="work-inspector-header">
            <div>
              <span className="eyebrow">Inspector</span>
              <strong>{inspectorMeta.title}</strong>
              <small>{inspectorMeta.hint}</small>
            </div>
            <button type="button" className="work-inspector-close" onClick={() => setInspector(null)} aria-label="Close inspector">
              ×
            </button>
          </header>
          <div className="work-inspector-body">
            {inspector === "contract" ? (
              <WorkItemContractCard />
            ) : inspector === "context" ? (
              <ContextInspectorCard />
            ) : (
              <WorkIntelligenceCard />
            )}
          </div>
        </aside>
      ) : null}
    </div>
  );
}
