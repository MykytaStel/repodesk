import { lazy, Suspense, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as api from "../../shared/api/orchestrate";
import { callCommand } from "../../shared/api/queries";
import type { ExecutionMode, Phase, PhaseProgress, PhaseStatus } from "../../shared/api/orchestrate";
import type { TabId } from "../../shared/types/api";

// The advanced orchestrator surface (plan/run/diff/history) is reused verbatim
// inside a collapsible panel — the Work tab leads with the single phase flow and
// keeps the full controls one disclosure away.
const OrchestrateTab = lazy(() =>
  import("../orchestrate/OrchestrateTab").then((m) => ({ default: m.OrchestrateTab })),
);

const PHASE_GLYPH: Record<PhaseStatus, string> = {
  done: "✓",
  in_progress: "◐",
  available: "→",
  locked: "•",
};

export function WorkTab({ setActiveTab }: { setActiveTab: (tab: TabId) => void }) {
  const queryClient = useQueryClient();
  const [showAdvanced, setShowAdvanced] = useState(false);

  const phase = useQuery({
    queryKey: ["work", "phase-state"],
    queryFn: () => api.workPhaseState(),
    refetchInterval: 4000,
  });

  const setMode = useMutation({
    mutationFn: (mode: ExecutionMode) => api.workSetExecutionMode(mode),
    onSuccess: (next) => queryClient.setQueryData(["work", "phase-state"], next),
  });

  const runCta = useMutation({
    mutationFn: async (actionId: string) => {
      await callCommand("run_desktop_action", { actionId });
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["work", "phase-state"] });
      void queryClient.invalidateQueries();
    },
  });

  // Explicit Review/Finish transitions (acks keyed to the latest run id).
  const transition = useMutation({
    mutationFn: (kind: "reviewed" | "committed") =>
      kind === "reviewed" ? api.workMarkReviewed() : api.workMarkCommitted(),
    onSuccess: (next) => queryClient.setQueryData(["work", "phase-state"], next),
  });

  if (phase.isLoading || !phase.data) {
    return (
      <div className="content-grid">
        <section className="hero-panel wide-panel">
          <p className="eyebrow">Work</p>
          <h2>Loading task flow…</h2>
        </section>
      </div>
    );
  }

  const progress: PhaseProgress = phase.data;
  const current = progress.phases.find((p) => p.phase === progress.current) ?? progress.phases[0];

  // The single primary action: run the phase's backing desktop action when it has
  // one, otherwise route the user to the surface where that phase is performed.
  function handlePrimary() {
    const { action_id, phase: ctaPhase } = progress.cta;
    if (action_id) {
      runCta.mutate(action_id);
      return;
    }
    switch (ctaPhase) {
      case "scope":
        setActiveTab("settings");
        break;
      case "execute":
        setShowAdvanced(true);
        break;
      case "review":
        setShowAdvanced(true);
        break;
      case "finish":
        setActiveTab("changes");
        break;
      default:
        setShowAdvanced(true);
    }
  }

  const inExecute = progress.current === "execute";

  return (
    <div className="content-grid work-tab">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Work</p>
        <h2>{progress.complete ? "Task complete" : current.title}</h2>
        <p className="muted">{current.summary}</p>

        {/* Phase rail — the six phases in order, with status. */}
        <ol className="phase-rail" aria-label="Task phases">
          {progress.phases.map((view) => (
            <li
              key={view.phase}
              className={`phase-chip phase-${view.status}${
                view.phase === progress.current ? " phase-current" : ""
              }`}
            >
              <span className="phase-glyph" aria-hidden="true">
                {PHASE_GLYPH[view.status]}
              </span>
              <span className="phase-title">{view.title}</span>
            </li>
          ))}
        </ol>

        {/* Execution mode lives in the Execute phase only. */}
        {inExecute && (
          <div className="execution-mode" role="group" aria-label="Execution mode">
            <ModeButton
              label="Agent run"
              hint="RepoDesk launches Codex/Claude in an isolated worktree"
              active={progress.execution_mode === "agent_run"}
              onClick={() => setMode.mutate("agent_run")}
            />
            <ModeButton
              label="Manual handoff"
              hint="Generate a context pack to copy to an external agent"
              active={progress.execution_mode === "manual_handoff"}
              onClick={() => setMode.mutate("manual_handoff")}
            />
          </div>
        )}

        {/* The one primary CTA, plus the explicit transition for the phase that
            needs a human decision (review / commit). */}
        <div className="work-cta-row">
          <button
            className="primary-cta"
            onClick={handlePrimary}
            disabled={progress.complete || runCta.isPending}
          >
            {runCta.isPending ? "Working…" : progress.cta.label}
          </button>
          {progress.current === "review" && (
            <button
              className="secondary-cta"
              onClick={() => transition.mutate("reviewed")}
              disabled={transition.isPending}
            >
              Mark changes reviewed
            </button>
          )}
          {progress.current === "finish" && (
            <button
              className="secondary-cta"
              onClick={() => transition.mutate("committed")}
              disabled={transition.isPending}
            >
              Mark committed
            </button>
          )}
        </div>
      </section>

      {/* Advanced orchestrator details, collapsed by default. */}
      <section className="panel wide-panel">
        <button
          className="disclosure"
          aria-expanded={showAdvanced}
          onClick={() => setShowAdvanced((open) => !open)}
        >
          {showAdvanced ? "▾" : "▸"} Advanced orchestrator details
        </button>
        {showAdvanced && (
          <Suspense fallback={<p className="muted">Loading orchestrator…</p>}>
            <OrchestrateTab setActiveTab={setActiveTab} />
          </Suspense>
        )}
      </section>
    </div>
  );
}

function ModeButton({
  label,
  hint,
  active,
  onClick,
}: {
  label: string;
  hint: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button className={`mode-button${active ? " selected" : ""}`} onClick={onClick} aria-pressed={active}>
      <span className="mode-label">{label}</span>
      <span className="mode-hint">{hint}</span>
    </button>
  );
}
