import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as api from "../../shared/api/orchestrate";
import * as strategyApi from "../../shared/api/strategy";
import { invalidateQueryDomains } from "../../shared/api/cacheInvalidation";
import { callCommand } from "../../shared/api/queries";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { useGit } from "../git/useGit";
import { codeChangedFiles } from "../../shared/utils/helpers";
import {
  ActionBar,
  ErrorState,
  LoadingState,
  Metric,
  PanelHeader,
  StatusBadge,
} from "../../shared/ui/primitives";
import { TaskSwitcher } from "../workflow/TaskSwitcher";
import { PromptsPanel } from "../workflow/PromptsPanel";
import { ExecutionPreviewCompact } from "./ExecutionPreviewCompact";
import { ExecutionStrategyControls } from "./ExecutionStrategyControls";
import { ReviewPanel } from "./ReviewPanel";
import { launchApprovalSemantic, phaseStatusSemantic } from "./workSemantic";
import type { Phase, PhaseProgress, PhaseStatus } from "../../shared/api/orchestrate";
import type { AiStrategyMode } from "../../shared/api/strategy";
import type { TabId } from "../../shared/types/api";

const PHASE_KEY = ["work", "phase-state"] as const;
const LATEST_RUN_KEY = ["work", "latest-run"] as const;

type ExecutionApproval = {
  fingerprint: string;
  approveCodingAgents: boolean;
  approvePaid: boolean;
};

const PHASE_COPY: Record<Phase, { detail: string; next: string }> = {
  scope: {
    detail: "Bind one project and one Work Item before RepoDesk prepares or delegates anything.",
    next: "Prepare builds the bounded context pack.",
  },
  prepare: {
    detail: "Build bounded context, run safety checks and determine how this Work Item should be executed.",
    next: "Execute hands only that prepared packet to the selected worker.",
  },
  execute: {
    detail: "Run an approved coding agent in an isolated workspace, or use a manual handoff and import the result.",
    next: "Review decides whether the exact returned ChangeSet is accepted.",
  },
  review: {
    detail: "Inspect the exact ChangeSet and decide whether those files can enter the staged review set.",
    next: "Accepted changes advance to receipt-bound verification.",
  },
  verify: {
    detail: "Run configured checks against the reviewed tree and record verification evidence bound to it.",
    next: "Only a fresh verified review can advance to Finish.",
  },
  finish: {
    detail: "Commit only the reviewed staged ChangeSet. RepoDesk never substitutes a blanket git add -A.",
    next: "The commit and run evidence remain available in Runs and Project Knowledge.",
  },
};

function PhaseStatusIcon({ status }: { status: PhaseStatus }) {
  switch (status) {
    case "done":
      return <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m3.2 8.2 3 3 6.6-6.8" /></svg>;
    case "in_progress":
      return <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 2.5a5.5 5.5 0 1 1-5.5 5.5" /><path d="M2.5 8H5" /></svg>;
    case "available":
      return <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8h9" /><path d="m9 5 3 3-3 3" /></svg>;
    case "locked":
      return <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="4" y="7" width="8" height="6" rx="1.4" /><path d="M6 7V5.5a2 2 0 0 1 4 0V7" /></svg>;
  }
}

export function WorkTab({ setActiveTab }: { setActiveTab: (tab: TabId) => void }) {
  const queryClient = useQueryClient();
  const { hasProject, hasTask, projectName } = useWorkspace();
  const { git } = useGit();
  const [executionApproval, setExecutionApproval] = useState<ExecutionApproval | null>(null);
  const [strategyMode, setStrategyMode] = useState<AiStrategyMode>("auto");
  const [commitMessage, setCommitMessage] = useState("");
  const [importPatch, setImportPatch] = useState("");

  const phase = useQuery({
    queryKey: PHASE_KEY,
    queryFn: () => api.workPhaseState(),
    staleTime: 1_500,
    refetchOnWindowFocus: true,
  });
  const latestRun = useQuery({
    queryKey: LATEST_RUN_KEY,
    queryFn: () => api.orchestrateStatus(),
    staleTime: 1_500,
    refetchOnWindowFocus: true,
  });
  const executePreview = useQuery({
    queryKey: ["work", "exec-preview", strategyMode],
    queryFn: () => strategyApi.workStrategyExecutionPreview(strategyMode),
    enabled: phase.data?.current === "execute" && phase.data?.execution_mode === "agent_run",
    staleTime: 5_000,
  });

  const setPhase = (next: PhaseProgress) => queryClient.setQueryData(PHASE_KEY, next);
  const refreshWork = (domains: Parameters<typeof invalidateQueryDomains>[1]) =>
    invalidateQueryDomains(queryClient, domains);

  const setMode = useMutation({
    mutationFn: (mode: api.ExecutionMode) => api.workSetExecutionMode(mode),
    onSuccess: (next) => {
      setPhase(next);
      setExecutionApproval(null);
      void queryClient.invalidateQueries({ queryKey: ["work", "exec-preview"] });
    },
  });

  const runCta = useMutation({
    mutationFn: (actionId: string) => callCommand("run_desktop_action", { actionId }),
    onSuccess: () => void refreshWork(["work", "runs"]),
  });

  const runAgent = useMutation({
    mutationFn: () => {
      const planFingerprint = executePreview.data?.plan_fingerprint ?? null;
      const approvalMatches = Boolean(
        planFingerprint && executionApproval?.fingerprint === planFingerprint,
      );
      return strategyApi.orchestrateStrategyRun({
        strategyMode,
        expectedPlanFingerprint: planFingerprint,
        approvalPlanFingerprint: approvalMatches ? executionApproval?.fingerprint ?? null : null,
        approvePaid: approvalMatches ? executionApproval?.approvePaid ?? false : false,
        approveCodingAgents: approvalMatches
          ? executionApproval?.approveCodingAgents ?? false
          : false,
      });
    },
    onSuccess: (run) => {
      queryClient.setQueryData(LATEST_RUN_KEY, run);
      setExecutionApproval(null);
      void refreshWork(["work", "git", "code", "runs"]);
    },
  });

  const review = useMutation({
    mutationFn: (action: api.ReviewAction) => {
      const runId = latestRun.data?.run_id;
      if (!runId) throw new Error("No run to review yet.");
      return api.workReview(runId, action);
    },
    onSuccess: (next) => {
      setPhase(next);
      void refreshWork(["work", "git", "code", "runs"]);
    },
  });

  const verify = useMutation({
    mutationFn: () => api.workVerify(),
    onSuccess: (next) => {
      setPhase(next);
      void refreshWork(["work", "runs"]);
    },
  });

  const importManual = useMutation({
    mutationFn: (patch: string | null) => api.workImportManualChanges(patch),
    onSuccess: (next) => {
      setImportPatch("");
      setPhase(next);
      void refreshWork(["work", "git", "code", "runs"]);
    },
  });

  const commit = useMutation({
    mutationFn: async (message: string) => {
      const result = await callCommand<{ ok: boolean; stderr: string }>("commit_ready_changes", { message });
      if (!result.ok) throw new Error(result.stderr || "Commit failed.");
      return api.workPhaseState();
    },
    onSuccess: (next) => {
      setCommitMessage("");
      setPhase(next);
      void refreshWork(["work", "git", "code", "runs"]);
    },
  });

  if (phase.isError) {
    const detail = phase.error instanceof Error ? phase.error.message : String(phase.error);
    return (
      <div className="work-phase-error">
        <ErrorState
          scope="surface"
          title="RepoDesk stopped instead of guessing"
          detail={(
            <>
              <p>The current workflow evidence could not be read safely. Progress is intentionally hidden until the evidence is valid again.</p>
              <code>{detail}</code>
            </>
          )}
          action={(
            <ActionBar
              primary={<button className="primary-cta" onClick={() => void phase.refetch()}>Retry</button>}
              secondary={<button className="secondary-cta" onClick={() => setActiveTab("history")}>Open Runs</button>}
            />
          )}
        />
      </div>
    );
  }

  if (phase.isLoading || !phase.data) {
    return <LoadingState scope="surface" message="Loading Work Item flow…" />;
  }

  const progress = phase.data;
  const current = progress.phases.find((item) => item.phase === progress.current) ?? progress.phases[0];
  const copy = PHASE_COPY[progress.current];
  const currentSummary = progress.current === "scope"
    ? !hasProject
      ? "Choose the repository for this Work Item"
      : !hasTask
        ? "Define the Work Item for this project"
        : "Project and Work Item are ready"
    : current.summary;
  const sharedPrimaryOwner =
    progress.current === "prepare" || progress.current === "execute" || progress.current === "verify";
  const isAgentRun = progress.execution_mode === "agent_run";
  const latest = latestRun.data ?? null;
  const changedCount = latest
    ? latest.results.reduce((count, result) => count + (result.changed_files?.length ?? 0), 0)
    : 0;
  const busy =
    runCta.isPending || runAgent.isPending || review.isPending || verify.isPending || importManual.isPending || commit.isPending;
  const strategyPreview = executePreview.data ?? null;
  const preview = strategyPreview?.execution ?? null;
  const planFingerprint = strategyPreview?.plan_fingerprint ?? null;
  const approvalMatchesPreview = Boolean(
    planFingerprint && executionApproval?.fingerprint === planFingerprint,
  );
  const approveCodingAgents = approvalMatchesPreview
    ? executionApproval?.approveCodingAgents ?? false
    : false;
  const approvePaid = approvalMatchesPreview ? executionApproval?.approvePaid ?? false : false;
  const approvalsStale = Boolean(
    executionApproval && planFingerprint && executionApproval.fingerprint !== planFingerprint,
  );
  const executeApprovalsMet =
    Boolean(preview) &&
    (!preview?.requires_coding_agent_approval || approveCodingAgents) &&
    (!preview?.requires_paid_approval || approvePaid);
  const approvalSemantic = launchApprovalSemantic({ stale: approvalsStale, ready: executeApprovalsMet });
  const executeBlocked =
    progress.current === "execute" &&
    isAgentRun &&
    (!executeApprovalsMet || executePreview.isLoading || executePreview.isFetching || executePreview.isError);

  const mutationError =
    (runAgent.error as Error | null)?.message ??
    (review.error as Error | null)?.message ??
    (verify.error as Error | null)?.message ??
    (importManual.error as Error | null)?.message ??
    (commit.error as Error | null)?.message ??
    null;

  function changeStrategy(next: AiStrategyMode) {
    setExecutionApproval(null);
    setStrategyMode(next);
  }

  function updateExecutionApproval(kind: "coding" | "paid", checked: boolean) {
    const fingerprint = executePreview.data?.plan_fingerprint;
    if (!fingerprint) return;
    setExecutionApproval((currentApproval) => {
      const base: ExecutionApproval = currentApproval?.fingerprint === fingerprint
        ? currentApproval
        : {
            fingerprint,
            approveCodingAgents: false,
            approvePaid: false,
          };
      return kind === "coding"
        ? { ...base, approveCodingAgents: checked }
        : { ...base, approvePaid: checked };
    });
  }

  function handlePrimary() {
    const { action_id: actionId, phase: ctaPhase } = progress.cta;
    if (ctaPhase === "execute" && isAgentRun) {
      runAgent.mutate();
      return;
    }
    if (ctaPhase === "verify") {
      verify.mutate();
      return;
    }
    if (actionId) {
      runCta.mutate(actionId);
      return;
    }
    setActiveTab("orchestrate");
  }

  return (
    <div className="work-tab work-focus-layout">
      <section className="work-focus-card">
        <div className="work-phase-header">
          <PanelHeader
            eyebrow="Current step"
            title={progress.complete ? "Task complete" : current.title}
            description={currentSummary}
            trailing={latest && !latest.dry_run ? (
              <div className="work-run-facts" aria-label="Latest run facts">
                <Metric label="Files" value={String(changedCount)} />
                <Metric label="Tokens" value={(latest.total_input_tokens + latest.total_output_tokens).toLocaleString()} />
                <Metric label="Cost" value={latest.total_cost_units.toFixed(3)} />
              </div>
            ) : null}
          />
        </div>

        <ol className="phase-rail compact" aria-label="Task phases">
          {progress.phases.map((view) => {
            const semantic = phaseStatusSemantic(view.status);
            return (
              <li
                key={view.phase}
                className={`phase-chip phase-${view.status}${view.phase === progress.current ? " phase-current" : ""}`}
                data-semantic-tone={semantic.tone}
                aria-label={`${view.title}: ${semantic.label}`}
              >
                <span className="phase-glyph" aria-hidden="true"><PhaseStatusIcon status={view.status} /></span>
                <span className="phase-title">{view.title}</span>
              </li>
            );
          })}
        </ol>

        <div className="work-current-step">
          <div className="work-current-step-copy">
            <strong>{copy.detail}</strong>
            <small>{copy.next}</small>
          </div>
        </div>

        {progress.current === "scope" ? (
          <div className="phase-controls compact">
            {!hasProject ? (
              <ActionBar
                primary={<button className="primary-cta" onClick={() => setActiveTab("projects")}>Connect a project</button>}
                detail="Bind the repository before RepoDesk prepares context or delegates work."
              />
            ) : !hasTask ? (
              <TaskSwitcher />
            ) : (
              <p className="muted">Project and Work Item are ready. Continue to Prepare.</p>
            )}
          </div>
        ) : null}

        {progress.current === "execute" ? (
          <div className="phase-controls compact">
            <div className="execution-mode compact" role="group" aria-label="Execution mode">
              <ModeButton
                label="Agent run"
                hint="Isolated coding-agent workspace"
                active={isAgentRun}
                onClick={() => setMode.mutate("agent_run")}
              />
              <ModeButton
                label="Manual handoff"
                hint="External agent, then import result"
                active={!isAgentRun}
                onClick={() => setMode.mutate("manual_handoff")}
              />
            </div>

            {isAgentRun ? (
              <>
                <ExecutionStrategyControls
                  mode={strategyMode}
                  preview={strategyPreview}
                  loading={executePreview.isLoading || executePreview.isFetching}
                  onModeChange={changeStrategy}
                />
                <ExecutionPreviewCompact
                  preview={preview}
                  loading={executePreview.isLoading || executePreview.isFetching}
                  error={(executePreview.error as Error | null)?.message ?? null}
                />
                <div className="exec-approval-gates">
                  <div className="exec-approval-heading">
                    <div>
                      <strong>Launch approvals</strong>
                      <small>
                        {approvalsStale
                          ? "The execution packet changed after approval. Re-approve the current plan lock before launch."
                          : "Nothing gated launches until you grant the matching capability for this exact plan lock."}
                      </small>
                    </div>
                    <StatusBadge label={approvalSemantic.label} tone={approvalSemantic.tone} />
                  </div>
                  <div className="approval-stack">
                    <label className={`approval-check${preview?.requires_coding_agent_approval && !approveCodingAgents ? " required" : ""}`}>
                      <input
                        type="checkbox"
                        checked={approveCodingAgents}
                        onChange={(event) => updateExecutionApproval("coding", event.target.checked)}
                      />
                      <span>
                        <strong>Coding agent + isolated writes</strong>
                        <small>{preview?.requires_coding_agent_approval ? "Allows the approved CLI to write only in its run worktree." : "Not required by this route."}</small>
                      </span>
                      {preview?.requires_coding_agent_approval ? <span className="approval-required">required</span> : null}
                    </label>
                    <label className={`approval-check${preview?.requires_paid_approval && !approvePaid ? " required" : ""}`}>
                      <input
                        type="checkbox"
                        checked={approvePaid}
                        onChange={(event) => updateExecutionApproval("paid", event.target.checked)}
                      />
                      <span>
                        <strong>Paid provider spend</strong>
                        <small>{preview?.requires_paid_approval ? "Allows the routed paid completion calls shown above." : "This packet does not require paid-provider approval."}</small>
                      </span>
                      {preview?.requires_paid_approval ? <span className="approval-required">required</span> : null}
                    </label>
                  </div>
                </div>
              </>
            ) : (
              <details className="work-secondary-details" open>
                <summary>Manual handoff packet and import</summary>
                <PromptsPanel />
                <div className="manual-import">
                  <textarea
                    id="manual-import-patch"
                    className="manual-import-input"
                    placeholder="Paste a unified diff, or leave empty and import changes already applied in the working tree."
                    value={importPatch}
                    onChange={(event) => setImportPatch(event.target.value)}
                    rows={5}
                  />
                  <ActionBar
                    secondary={(
                      <>
                        <button className="secondary-cta" onClick={() => importManual.mutate(importPatch)} disabled={importManual.isPending || !importPatch.trim()}>
                          Import diff → Review
                        </button>
                        <button className="secondary-cta" onClick={() => importManual.mutate(null)} disabled={importManual.isPending}>
                          Import working tree → Review
                        </button>
                      </>
                    )}
                  />
                </div>
              </details>
            )}
          </div>
        ) : null}

        {progress.current === "review" ? (
          <div className="phase-controls compact">
            <ReviewPanel runId={latest?.run_id ?? null} projectName={projectName} />
            <ActionBar
              primary={(
                <button className="primary-cta" onClick={() => review.mutate("accept")} disabled={review.isPending || !latest}>
                  Accept &amp; stage → Verify
                </button>
              )}
              destructive={(
                <button className="secondary-cta" onClick={() => review.mutate("reject")} disabled={review.isPending || !latest}>
                  Reject → re-run
                </button>
              )}
            />
          </div>
        ) : null}

        {progress.current === "finish" ? (
          <div className="phase-controls compact">
            <CommitFiles files={codeChangedFiles(git)} onView={() => setActiveTab("changes")} />
            <div className="work-commit-row">
              <ActionBar
                secondary={(
                  <input
                    className="commit-input"
                    placeholder="Commit message"
                    value={commitMessage}
                    onChange={(event) => setCommitMessage(event.target.value)}
                  />
                )}
                primary={(
                  <button
                    className="primary-cta"
                    onClick={() => commit.mutate(commitMessage.trim())}
                    disabled={commit.isPending || commitMessage.trim().length === 0}
                  >
                    Commit reviewed changes
                  </button>
                )}
              />
            </div>
          </div>
        ) : null}

        {sharedPrimaryOwner ? (
          <div className="work-cta-row focus">
            <ActionBar
              primary={(
                <button className="primary-cta" onClick={handlePrimary} disabled={progress.complete || busy || executeBlocked}>
                  {busy ? "Working…" : progress.cta.label}
                </button>
              )}
              detail={executeBlocked ? "Refresh the strategy packet and grant its required approvals before launch." : undefined}
            />
          </div>
        ) : null}

        {mutationError ? <ErrorState title="Work action failed" detail={mutationError} /> : null}
      </section>

      <section className="work-tools-strip">
        <div>
          <strong>Need lower-level control?</strong>
          <small>Routing, workers and multi-agent details live in Orchestrate so the main Work flow stays focused.</small>
        </div>
        <button className="secondary-cta" onClick={() => setActiveTab("orchestrate")}>Open Orchestrate</button>
      </section>
    </div>
  );
}

function CommitFiles({ files, onView }: { files: string[]; onView: () => void }) {
  if (files.length === 0) return <p className="muted">Working tree is clean — nothing to commit.</p>;
  return (
    <div className="commit-files compact">
      <p className="muted">
        {files.length} reviewed file{files.length === 1 ? "" : "s"}.{" "}
        <button className="link-cta" onClick={onView}>View diffs</button>
      </p>
      <ul className="commit-file-list compact">
        {files.slice(0, 8).map((file) => <li key={file}>{file}</li>)}
        {files.length > 8 ? <li className="muted">…and {files.length - 8} more</li> : null}
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
