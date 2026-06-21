import { lazy, Suspense, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as api from "../../shared/api/orchestrate";
import { callCommand } from "../../shared/api/queries";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { useGit } from "../git/useGit";
import { codeChangedFiles } from "../../shared/utils/helpers";
import { TaskSwitcher } from "../workflow/TaskSwitcher";
import { PromptsPanel } from "../workflow/PromptsPanel";
import { ReviewPanel } from "./ReviewPanel";
import type { PhaseProgress, PhaseStatus } from "../../shared/api/orchestrate";
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

const PHASE_KEY = ["work", "phase-state"] as const;

export function WorkTab({ setActiveTab }: { setActiveTab: (tab: TabId) => void }) {
  const queryClient = useQueryClient();
  const { hasProject, hasTask, projectName } = useWorkspace();
  const { git } = useGit();
  const [showAdvanced, setShowAdvanced] = useState(false);
  // Execute-phase approvals (the ExecutionAuthorization gates) and the commit
  // message live in the Work card so the flow never leaves this surface.
  const [approveCodingAgents, setApproveCodingAgents] = useState(false);
  const [approvePaid, setApprovePaid] = useState(false);
  const [commitMessage, setCommitMessage] = useState("");

  const phase = useQuery({
    queryKey: PHASE_KEY,
    queryFn: () => api.workPhaseState(),
    refetchInterval: 4000,
  });
  const latestRun = useQuery({
    queryKey: ["work", "latest-run"],
    queryFn: () => api.orchestrateStatus(),
  });

  const setPhase = (next: PhaseProgress) => queryClient.setQueryData(PHASE_KEY, next);
  const refreshAll = () => {
    void queryClient.invalidateQueries({ queryKey: ["work"] });
    void queryClient.invalidateQueries();
  };

  const setMode = useMutation({
    mutationFn: (mode: api.ExecutionMode) => api.workSetExecutionMode(mode),
    onSuccess: setPhase,
  });

  // Action-backed CTAs (e.g. build context, generate the manual context pack).
  const runCta = useMutation({
    mutationFn: (actionId: string) => callCommand("run_desktop_action", { actionId }),
    onSuccess: refreshAll,
  });

  // Execute (agent run): launch the orchestrator with the inline approvals.
  const runAgent = useMutation({
    mutationFn: () => api.orchestrateRun(undefined, false, null, approvePaid, approveCodingAgents),
    onSuccess: refreshAll,
  });

  // Review: accept/reject the last run's changeset, then record the ack.
  const review = useMutation({
    mutationFn: async (action: api.ReviewAction) => {
      const runId = latestRun.data?.run_id;
      if (!runId) throw new Error("No run to review yet.");
      await api.orchestrateReview(runId, action);
      return api.workMarkReviewed();
    },
    onSuccess: (next) => {
      setPhase(next);
      refreshAll();
    },
  });

  // Finish: commit (server re-checks readiness), then record the commit ack.
  const commit = useMutation({
    mutationFn: async (message: string) => {
      const result = await callCommand<{ ok: boolean; stderr: string }>("commit_ready_changes", {
        message,
      });
      if (!result.ok) throw new Error(result.stderr || "Commit failed.");
      return api.workMarkCommitted();
    },
    onSuccess: (next) => {
      setCommitMessage("");
      setPhase(next);
      refreshAll();
    },
  });

  // Fallback acks when no run-backed action applies (e.g. hand-edited changes).
  const ackReviewed = useMutation({ mutationFn: api.workMarkReviewed, onSuccess: setPhase });
  const ackCommitted = useMutation({ mutationFn: api.workMarkCommitted, onSuccess: setPhase });

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
  const isAgentRun = progress.execution_mode === "agent_run";
  const busy =
    runCta.isPending || runAgent.isPending || review.isPending || commit.isPending;

  const latest = latestRun.data ?? null;
  const changedCount = latest
    ? latest.results.reduce((n, r) => n + (r.changed_files?.length ?? 0), 0)
    : 0;

  // The single primary action for the current phase. Execute agent-runs launch
  // inline; action-backed CTAs dispatch their desktop action; the rest route.
  function handlePrimary() {
    const { action_id, phase: ctaPhase } = progress.cta;
    if (ctaPhase === "execute" && isAgentRun) {
      runAgent.mutate();
      return;
    }
    if (action_id) {
      runCta.mutate(action_id);
      return;
    }
    switch (ctaPhase) {
      case "scope":
        setActiveTab("settings");
        break;
      case "finish":
        setActiveTab("changes");
        break;
      default:
        setShowAdvanced(true);
    }
  }

  const mutationError =
    (runAgent.error as Error | null)?.message ??
    (review.error as Error | null)?.message ??
    (commit.error as Error | null)?.message ??
    null;

  return (
    <div className="content-grid work-tab">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Work</p>
        <h2>{progress.complete ? "Task complete" : current.title}</h2>
        <p className="muted">{current.summary}</p>
        {latest && !latest.dry_run && (
          <p className="muted work-cost">
            Last run: {latest.total_cost_units.toFixed(3)} cost units ·{" "}
            {(latest.total_input_tokens + latest.total_output_tokens).toLocaleString()} tokens
          </p>
        )}

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

        {/* Scope onboarding — connect a project, then create the task inline so
            the whole flow starts here (no separate onboarding surface). */}
        {progress.current === "scope" && (
          <div className="phase-controls">
            {!hasProject ? (
              <>
                <p className="muted">Connect a project to start a task.</p>
                <div className="phase-actions">
                  <button className="secondary-cta" onClick={() => setActiveTab("settings")}>
                    Connect a project
                  </button>
                </div>
              </>
            ) : !hasTask ? (
              <TaskSwitcher />
            ) : (
              <p className="muted">Project, task, and goal are set — continue to Prepare.</p>
            )}
          </div>
        )}

        {/* Execution mode + approvals live in the Execute phase only. */}
        {progress.current === "execute" && (
          <>
            <div className="execution-mode" role="group" aria-label="Execution mode">
              <ModeButton
                label="Agent run"
                hint="RepoDesk launches Codex/Claude in an isolated worktree"
                active={isAgentRun}
                onClick={() => setMode.mutate("agent_run")}
              />
              <ModeButton
                label="Manual handoff"
                hint="Generate a context pack to copy to an external agent"
                active={!isAgentRun}
                onClick={() => setMode.mutate("manual_handoff")}
              />
            </div>
            {isAgentRun ? (
              <div className="phase-controls" role="group" aria-label="Run approvals">
                <label className="approval-check">
                  <input
                    type="checkbox"
                    checked={approveCodingAgents}
                    onChange={(e) => setApproveCodingAgents(e.target.checked)}
                  />
                  Approve coding-agent CLIs (launch + workspace writes)
                </label>
                <label className="approval-check">
                  <input
                    type="checkbox"
                    checked={approvePaid}
                    onChange={(e) => setApprovePaid(e.target.checked)}
                  />
                  Approve paid providers (may spend)
                </label>
              </div>
            ) : (
              // Manual handoff: generate the pack with the CTA, then copy a prompt
              // here to hand to an external agent — no detour to a separate tab.
              <div className="phase-controls">
                <PromptsPanel />
              </div>
            )}
          </>
        )}

        {/* Review: see exactly what changed + the proposed memory, then decide. */}
        {progress.current === "review" && (
          <div className="phase-controls">
            <ReviewPanel runId={latest?.run_id ?? null} projectName={projectName} />
            <p className="muted">
              {changedCount > 0
                ? `${changedCount} changed file(s) from the last run.`
                : "No run changeset detected — review and stage your edits."}
            </p>
            <div className="phase-actions">
              <button
                className="secondary-cta"
                onClick={() => review.mutate("accept")}
                disabled={review.isPending || !latest}
              >
                Accept & stage
              </button>
              <button
                className="secondary-cta"
                onClick={() => review.mutate("reject")}
                disabled={review.isPending || !latest}
              >
                Reject
              </button>
              <button className="link-cta" onClick={() => ackReviewed.mutate()}>
                Mark reviewed
              </button>
            </div>
          </div>
        )}

        {/* Finish: show exactly what will be committed, then commit. */}
        {progress.current === "finish" && (
          <div className="phase-controls">
            <CommitFiles files={codeChangedFiles(git)} onView={() => setActiveTab("changes")} />
            <input
              className="commit-input"
              placeholder="Commit message"
              value={commitMessage}
              onChange={(e) => setCommitMessage(e.target.value)}
            />
            <div className="phase-actions">
              <button
                className="secondary-cta"
                onClick={() => commit.mutate(commitMessage.trim())}
                disabled={commit.isPending || commitMessage.trim().length === 0}
              >
                Commit changes
              </button>
              <button className="link-cta" onClick={() => ackCommitted.mutate()}>
                Mark committed
              </button>
            </div>
          </div>
        )}

        {/* The one primary CTA. */}
        <div className="work-cta-row">
          <button className="primary-cta" onClick={handlePrimary} disabled={progress.complete || busy}>
            {busy ? "Working…" : progress.cta.label}
          </button>
        </div>

        {mutationError && <p className="work-error">{mutationError}</p>}
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

function CommitFiles({ files, onView }: { files: string[]; onView: () => void }) {
  if (files.length === 0) {
    return <p className="muted">Working tree is clean — nothing to commit.</p>;
  }
  return (
    <div className="commit-files">
      <p className="muted">
        {files.length} file(s) will be committed:{" "}
        <button className="link-cta" onClick={onView}>
          view diffs
        </button>
      </p>
      <ul className="commit-file-list">
        {files.slice(0, 12).map((file) => (
          <li key={file}>{file}</li>
        ))}
        {files.length > 12 && <li className="muted">…and {files.length - 12} more</li>}
      </ul>
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
