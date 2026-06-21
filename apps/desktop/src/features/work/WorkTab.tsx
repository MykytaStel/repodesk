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
import type { PhaseProgress, PhaseStatus, ExecutionPreview } from "../../shared/api/orchestrate";
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
  // What the agent run would do, shown before launch. Only fetched while the
  // Execute phase is the agent-run target.
  const executePreview = useQuery({
    queryKey: ["work", "exec-preview"],
    queryFn: () => api.workExecutionPreview(),
    enabled: phase.data?.current === "execute" && phase.data?.execution_mode === "agent_run",
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

  // Review: accept (stage + record Accepted → Verify) or reject (discard +
  // record Rejected → re-open Execute), atomically server-side. No manual ack.
  const review = useMutation({
    mutationFn: (action: api.ReviewAction) => {
      const runId = latestRun.data?.run_id;
      if (!runId) throw new Error("No run to review yet.");
      return api.workReview(runId, action);
    },
    onSuccess: (next) => {
      setPhase(next);
      refreshAll();
    },
  });

  // Verify: run the receipt-bound verification (project checks against the
  // current HEAD + staged index + reviewed changeset).
  const verify = useMutation({
    mutationFn: () => api.workVerify(),
    onSuccess: (next) => {
      setPhase(next);
      refreshAll();
    },
  });

  // Finish: the bounded commit (server commits only the reviewed, staged index
  // and writes the finish receipt). No manual "mark committed".
  const commit = useMutation({
    mutationFn: async (message: string) => {
      const result = await callCommand<{ ok: boolean; stderr: string }>("commit_ready_changes", {
        message,
      });
      if (!result.ok) throw new Error(result.stderr || "Commit failed.");
      return api.workPhaseState();
    },
    onSuccess: (next) => {
      setCommitMessage("");
      setPhase(next);
      refreshAll();
    },
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
  const isAgentRun = progress.execution_mode === "agent_run";
  const busy =
    runCta.isPending ||
    runAgent.isPending ||
    review.isPending ||
    verify.isPending ||
    commit.isPending;

  // An agent run can't launch until the approvals the preview says it needs are
  // granted (the backend enforces this too; the gate just makes it legible).
  const preview = executePreview.data ?? null;
  const executeApprovalsMet =
    (!preview?.requires_coding_agent_approval || approveCodingAgents) &&
    (!preview?.requires_paid_approval || approvePaid);
  const executeBlocked =
    progress.current === "execute" && isAgentRun && !executeApprovalsMet;

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
    if (ctaPhase === "verify") {
      verify.mutate();
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
    (verify.error as Error | null)?.message ??
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
                label="Manual handoff (experimental)"
                hint="Preview only — generates a context pack; no return-import yet"
                active={!isAgentRun}
                onClick={() => setMode.mutate("manual_handoff")}
              />
            </div>
            {isAgentRun ? (
              <div className="phase-controls" role="group" aria-label="Run approvals">
                <ExecutionPreviewPanel
                  preview={executePreview.data ?? null}
                  loading={executePreview.isLoading}
                  error={(executePreview.error as Error | null)?.message ?? null}
                />
                <label
                  className={`approval-check${
                    executePreview.data?.requires_coding_agent_approval && !approveCodingAgents
                      ? " required"
                      : ""
                  }`}
                >
                  <input
                    type="checkbox"
                    checked={approveCodingAgents}
                    onChange={(e) => setApproveCodingAgents(e.target.checked)}
                  />
                  Approve coding-agent CLIs (launch + workspace writes)
                  {executePreview.data?.requires_coding_agent_approval && (
                    <span className="approval-required">required</span>
                  )}
                </label>
                <label
                  className={`approval-check${
                    executePreview.data?.requires_paid_approval && !approvePaid ? " required" : ""
                  }`}
                >
                  <input
                    type="checkbox"
                    checked={approvePaid}
                    onChange={(e) => setApprovePaid(e.target.checked)}
                  />
                  Approve paid providers (may spend)
                  {executePreview.data?.requires_paid_approval && (
                    <span className="approval-required">required</span>
                  )}
                </label>
              </div>
            ) : (
              // Manual handoff: generate the pack and continue in an external
              // agent. This mode is a preview — RepoDesk can't yet import the
              // returned changes as run evidence, so the six-phase flow won't
              // auto-advance past Execute until you switch back to an agent run.
              <div className="phase-controls">
                <div className="notice">
                  <strong>Experimental — preview only.</strong>
                  <ol className="manual-steps">
                    <li>Generate the context pack below.</li>
                    <li>Continue the work in your external agent.</li>
                    <li>Import / confirm the returned changes — <em>not implemented yet</em>.</li>
                  </ol>
                </div>
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
                Accept &amp; stage → Verify
              </button>
              <button
                className="secondary-cta"
                onClick={() => review.mutate("reject")}
                disabled={review.isPending || !latest}
              >
                Reject → re-run
              </button>
            </div>
            <p className="muted">
              Accept stages exactly this run's files and moves to Verify. Reject discards them and
              re-opens Execute — no changes are kept.
            </p>
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
                Commit reviewed changes
              </button>
            </div>
            <p className="muted">
              Commits only the reviewed, staged changeset — never a blanket <code>git add -A</code>.
            </p>
          </div>
        )}

        {/* The one primary CTA. */}
        <div className="work-cta-row">
          <button
            className="primary-cta"
            onClick={handlePrimary}
            disabled={progress.complete || busy || executeBlocked}
          >
            {busy ? "Working…" : progress.cta.label}
          </button>
          {executeBlocked && (
            <span className="muted">Grant the required approvals above to launch.</span>
          )}
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

function ExecutionPreviewPanel({
  preview,
  loading,
  error,
}: {
  preview: ExecutionPreview | null;
  loading: boolean;
  error: string | null;
}) {
  if (loading) return <p className="muted">Estimating the run…</p>;
  if (error) return <p className="muted">Couldn't build a run preview: {error}</p>;
  if (!preview || preview.steps.length === 0) return null;

  // Headline on the write (implementation) step when there is one, else the last.
  const lead =
    preview.steps.find((s) => s.allow_write) ?? preview.steps[preview.steps.length - 1];

  return (
    <div className="exec-preview">
      <p className="eyebrow">Before you launch</p>
      <dl className="exec-preview-grid">
        {lead && (
          <>
            <dt>Executor</dt>
            <dd>{lead.executor_label}</dd>
            <dt>Model</dt>
            <dd>{lead.model}</dd>
          </>
        )}
        <dt>Workspace</dt>
        <dd>{preview.isolated_workspace ? "Isolated worktree" : "Active checkout"}</dd>
        <dt>Expected writes</dt>
        <dd>{preview.expected_writes ? "Yes" : "No"}</dd>
        <dt>Estimated tokens</dt>
        <dd>{preview.total_estimated_tokens.toLocaleString()}</dd>
        <dt>Estimated cost</dt>
        <dd>
          {preview.total_estimated_cost_units.toFixed(2)} {preview.currency_label}
        </dd>
      </dl>

      {preview.steps.length > 1 && (
        <ul className="exec-preview-steps">
          {preview.steps.map((s) => (
            <li key={s.step_id}>
              <code>{s.title}</code> — {s.executor_label} · {s.model}
              {s.allow_write && <span className="pill warn">writes</span>}
              {s.paid && <span className="pill">paid</span>}
            </li>
          ))}
        </ul>
      )}

      <div className="exec-preview-approvals">
        <span className="eyebrow">Required approvals</span>
        {!preview.requires_coding_agent_approval && !preview.requires_paid_approval ? (
          <span className="muted"> none — local, no writes</span>
        ) : (
          <ul>
            {preview.requires_coding_agent_approval && (
              <li>Coding-agent process + workspace writes</li>
            )}
            {preview.requires_paid_approval && <li>Paid providers (may spend)</li>}
          </ul>
        )}
      </div>
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
