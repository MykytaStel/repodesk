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
import type { Phase, PhaseProgress, PhaseStatus, ExecutionPreview } from "../../shared/api/orchestrate";
import type { TabId } from "../../shared/types/api";
import { ChevronIcon } from "../../app/NavIcons";

// The advanced orchestrator surface (plan/run/diff/history) is reused verbatim
// inside a collapsible panel — the Work tab leads with the single phase flow and
// keeps the full controls one disclosure away.
const OrchestrateTab = lazy(() =>
  import("../orchestrate/OrchestrateTab").then((m) => ({ default: m.OrchestrateTab })),
);

const PHASE_KEY = ["work", "phase-state"] as const;

type PhaseBriefInfo = {
  input: string;
  action: string;
  output: string;
  next: string;
};

const PHASE_BRIEFS: Record<Phase, PhaseBriefInfo> = {
  scope: {
    input: "Project + one active task.",
    action: "RepoDesk locks the work to this repository and task.",
    output: "A bounded task record.",
    next: "Prepare builds the context pack.",
  },
  prepare: {
    input: "Task, git state, configured memory, and safety rules.",
    action: "RepoDesk builds bounded context, scans it, and routes the task.",
    output: "Context pack + route/check receipts.",
    next: "Execute can hand this packet to an agent.",
  },
  execute: {
    input: "Task goal, bounded context, memory slice, route, and approvals.",
    action: "RepoDesk launches the selected agent/executor with that packet.",
    output: "Run receipt, changed files, diff, and memory proposals.",
    next: "Review decides whether those changes enter the active checkout.",
  },
  review: {
    input: "Agent run receipt, diff, changed files, and proposed memory.",
    action: "You accept or reject the exact changeset.",
    output: "Accepted files are staged; rejected files are discarded or left isolated.",
    next: "Verify runs project checks against the reviewed tree.",
  },
  verify: {
    input: "Reviewed/staged changeset and the run receipt.",
    action: "RepoDesk runs the configured verification checks.",
    output: "Proof receipt bound to the current tree.",
    next: "Finish commits only the reviewed staged changes.",
  },
  finish: {
    input: "Verified staged files and your commit message.",
    action: "RepoDesk commits the reviewed changeset only.",
    output: "A git commit and closed task receipt.",
    next: "History/Outcomes keep the run evidence.",
  },
};

function PhaseStatusIcon({ status }: { status: PhaseStatus }) {
  switch (status) {
    case "done":
      return (
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="m3.2 8.2 3 3 6.6-6.8" />
        </svg>
      );
    case "in_progress":
      return (
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="M8 2.5a5.5 5.5 0 1 1-5.5 5.5" />
          <path d="M2.5 8H5" />
        </svg>
      );
    case "available":
      return (
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="M3 8h9" />
          <path d="m9 5 3 3-3 3" />
        </svg>
      );
    case "locked":
      return (
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <rect x="4" y="7" width="8" height="6" rx="1.4" />
          <path d="M6 7V5.5a2 2 0 0 1 4 0V7" />
        </svg>
      );
  }
}

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
  // Manual-handoff return-import: an optional pasted unified diff (empty = import
  // whatever the human already changed in the working tree).
  const [importPatch, setImportPatch] = useState("");

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

  // Manual handoff: import the external agent's result (pasted diff or the
  // already-applied working-tree changes) as run evidence → advances to Review.
  const importManual = useMutation({
    mutationFn: (patch: string | null) => api.workImportManualChanges(patch),
    onSuccess: (next) => {
      setImportPatch("");
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
    importManual.isPending ||
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
    (importManual.error as Error | null)?.message ??
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
                <PhaseStatusIcon status={view.status} />
              </span>
              <span className="phase-title">{view.title}</span>
            </li>
          ))}
        </ol>

        <PhaseBrief
          phase={progress.current}
          ctaLabel={progress.cta.label}
          isAgentRun={isAgentRun}
          preview={preview}
          changedCount={changedCount}
          busy={busy}
        />

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
                hint="Generate a context pack, run it in an external agent, then import the result"
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
              // Manual handoff: generate the pack, run it in an external agent,
              // then import the result back as run evidence so the six-phase flow
              // advances to Review. The import is secret-scanned before it counts.
              <div className="phase-controls">
                <div className="notice">
                  <strong>Manual handoff</strong>
                  <ol className="manual-steps">
                    <li>Generate the context pack below.</li>
                    <li>Continue the work in your external agent.</li>
                    <li>Import the returned changes to review and commit them here.</li>
                  </ol>
                </div>
                <PromptsPanel />
                <div className="manual-import">
                  <label htmlFor="manual-import-patch" className="eyebrow">
                    Import result
                  </label>
                  <textarea
                    id="manual-import-patch"
                    className="manual-import-input"
                    placeholder="Paste a unified diff (git diff) here — or leave empty to import the changes you already applied in the working tree."
                    value={importPatch}
                    onChange={(e) => setImportPatch(e.target.value)}
                    rows={6}
                  />
                  <div className="phase-actions">
                    <button
                      className="secondary-cta"
                      onClick={() => importManual.mutate(importPatch)}
                      disabled={importManual.isPending || !importPatch.trim()}
                    >
                      Import pasted diff → Review
                    </button>
                    <button
                      className="secondary-cta"
                      onClick={() => importManual.mutate(null)}
                      disabled={importManual.isPending}
                    >
                      Import working-tree changes → Review
                    </button>
                  </div>
                  {importManual.error && (
                    <p className="notice danger">
                      {(importManual.error as Error).message}
                    </p>
                  )}
                </div>
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
          {runAgent.isPending && (
            <span className="pill warn">Launching isolated agent run</span>
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
          <span className="disclosure-icon" aria-hidden="true">
            <ChevronIcon open={showAdvanced} />
          </span>
          Advanced orchestrator details
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

function PhaseBrief({
  phase,
  ctaLabel,
  isAgentRun,
  preview,
  changedCount,
  busy,
}: {
  phase: Phase;
  ctaLabel: string;
  isAgentRun: boolean;
  preview: ExecutionPreview | null;
  changedCount: number;
  busy: boolean;
}) {
  const brief = PHASE_BRIEFS[phase];
  const executeDetail =
    phase === "execute"
      ? isAgentRun
        ? preview
          ? `${preview.steps.length} step${preview.steps.length === 1 ? "" : "s"} routed; ${
              preview.isolated_workspace ? "isolated worktree" : "active checkout"
            }; ${preview.expected_writes ? "writes expected" : "read-only"}`
          : "Agent route is still loading."
        : "Manual handoff builds a context pack only; RepoDesk cannot import the result yet."
      : phase === "review"
        ? `${changedCount} changed file${changedCount === 1 ? "" : "s"} from the latest run.`
        : brief.output;

  return (
    <div className="phase-brief" aria-label="Current phase consequence">
      <div className="phase-brief-head">
        <span className={`pill ${busy ? "warn" : "accent"}`}>{busy ? "Running" : ctaLabel}</span>
        <strong>{phase === "execute" && isAgentRun ? "Agent handoff" : "Current step"}</strong>
      </div>
      <div className="phase-brief-grid">
        <BriefCell label="Input" value={brief.input} />
        <BriefCell label="RepoDesk does" value={brief.action} />
        <BriefCell label="Result" value={executeDetail} />
        <BriefCell label="Then" value={brief.next} />
      </div>
    </div>
  );
}

function BriefCell({ label, value }: { label: string; value: string }) {
  return (
    <div className="phase-brief-cell">
      <span>{label}</span>
      <strong>{value}</strong>
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

      <div className="agent-packet">
        <div>
          <span>Sent to agent</span>
          <strong>Goal + bounded context + memory slice + step instruction + token budget</strong>
        </div>
        <div>
          <span>Not sent</span>
          <strong>Repo-wide raw file dump; content must pass context and secret gates first</strong>
        </div>
        <div>
          <span>Comes back</span>
          <strong>Run receipt, changed-file list, diff path, notes, and memory proposals</strong>
        </div>
      </div>

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
