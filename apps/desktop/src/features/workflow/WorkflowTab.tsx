import React, { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { asArray, asRecord, getString, stringifyPreview, formatNumber, formatCost, statusTone, RouteList } from "../../shared/ui/SharedComponents";
import { projectAdd, projectUse, taskNew } from "../../shared/api/workflow";
import { pickDirectory, basename } from "../../shared/api/dialog";
import { useWorkflow } from "./useWorkflow";
import { TaskSwitcher } from "./TaskSwitcher";
import { useRouting } from "../routing/useRouting";
import { useTokens } from "../tokens/useTokens";
import { useGit } from "../git/useGit";
import { PromptsPanel } from "./PromptsPanel";
import { DiffViewerModal } from "../git/DiffViewerModal";

interface WorkflowTabProps {
  economyMode: string;
}

export function WorkflowTab({ economyMode }: WorkflowTabProps) {
  const { workflow, nextAction, doNextSafeStep, isRunning, history, commitChanges, isCommitting, commitError } = useWorkflow();
  const { routing } = useRouting(economyMode);
  const { tokens } = useTokens();
  const { git, dirty } = useGit();
  const queryClient = useQueryClient();
  const isBusy = isRunning;
  const lastResult = ((history[0] as any)?.result as unknown) ?? null;
  const steps = asArray(getValue(workflow, "steps"));

  const projectOk = Boolean(getValue(workflow, "project_ok"));
  const taskOk = Boolean(getValue(workflow, "task_ok"));
  const promptsOk = Boolean(getValue(workflow, "prompts_ok"));
  const needsOnboarding = !projectOk || !taskOk;

  const [commitMessage, setCommitMessage] = useState("");
  const [confirmCommit, setConfirmCommit] = useState(false);
  const [onboardError, setOnboardError] = useState("");
  const [projName, setProjName] = useState("");
  const [projPath, setProjPath] = useState("");
  const [projType, setProjType] = useState("rust");
  const [taskTitle, setTaskTitle] = useState("");
  const [diffFile, setDiffFile] = useState<{ path: string; cached: boolean } | null>(null);

  function getValue(source: unknown, key: string): unknown {
    return asRecord(source)[key];
  }

  async function browseForPath() {
    const path = await pickDirectory();
    if (!path) return;
    setProjPath(path);
    if (!projName.trim()) setProjName(basename(path));
  }

  async function addProject() {
    setOnboardError("");
    try {
      const input = { name: projName.trim(), path: projPath.trim(), project_type: projType.trim() || "generic", main_language: null };
      const added = await projectAdd(input);
      const alreadyExists = !added.ok && /already exists/i.test(added.stderr);
      if (!added.ok && !alreadyExists) {
        throw new Error(added.stderr || "Could not connect project.");
      }

      const activated = await projectUse(input.name);
      if (!activated.ok) {
        throw new Error(activated.stderr || "Project was connected, but could not be activated.");
      }
      setProjName(""); setProjPath("");
      await queryClient.invalidateQueries();
    } catch (error: any) {
      setOnboardError(error?.message || String(error));
    }
  }

  async function createTask() {
    setOnboardError("");
    try {
      await taskNew(taskTitle.trim());
      setTaskTitle("");
      await queryClient.invalidateQueries();
    } catch (error: any) {
      setOnboardError(error?.message || String(error));
    }
  }

  async function runCommit() {
    try {
      await commitChanges(commitMessage.trim());
      setCommitMessage("");
      setConfirmCommit(false);
    } catch {
      // commitError is surfaced from the hook below.
    }
  }

  function renderOnboarding() {
    return (
      <section className="panel onboarding wide-panel">
        <p className="eyebrow">Get started</p>
        <h2>{!projectOk ? "Step 1 · Connect a project" : "Step 2 · Create a task"}</h2>
        <p className="lead">RepoDesk works one task at a time. Connect a project, then create the task you want to work on.</p>
        {!projectOk ? (
          <div className="form-grid">
            <label>Name<input value={projName} onChange={(e) => setProjName(e.target.value)} placeholder="my-app" /></label>
            <label>Path
              <div className="input-with-action">
                <input value={projPath} onChange={(e) => setProjPath(e.target.value)} placeholder="/Users/you/code/my-app" />
                <button type="button" className="ghost-button" onClick={() => void browseForPath()}>Browse…</button>
              </div>
            </label>
            <label>Type<input value={projType} onChange={(e) => setProjType(e.target.value)} placeholder="rust" /></label>
            <button className="primary-button" disabled={!projName.trim() || !projPath.trim()} onClick={() => void addProject()}>Connect project</button>
          </div>
        ) : (
          <div className="form-grid">
            <label>Task title<input value={taskTitle} onChange={(e) => setTaskTitle(e.target.value)} placeholder="Add login rate-limit" /></label>
            <button className="primary-button" disabled={!taskTitle.trim()} onClick={() => void createTask()}>Create task</button>
          </div>
        )}
        {onboardError && <div className="notice danger">{onboardError}</div>}
      </section>
    );
  }

  function renderCommitReadiness() {
    const readiness = asRecord(getValue(workflow, "commit_readiness"));
    const status = getString(readiness, "status", "");
    if (!status) return null;

    const tone: "ok" | "warn" | "danger" | "neutral" =
      status === "ready" ? "ok" : status === "warning" ? "warn" : status === "blocked" ? "danger" : "neutral";
    const label =
      status === "ready" ? "Ready to commit"
      : status === "warning" ? "Review before commit"
      : status === "blocked" ? "Commit blocked"
      : status === "nothing_to_commit" ? "Nothing to commit"
      : "Not a Git repo";

    const blockers = asArray(getValue(readiness, "blockers")).map((item) => String(item));
    const warnings = asArray(getValue(readiness, "warnings")).map((item) => String(item));
    const branch = getString(readiness, "branch", "");
    const changed = Number(getValue(readiness, "changed_count") ?? 0);
    const canCommit = status === "ready" || status === "warning";

    return (
      <section className="panel commit-readiness wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Commit readiness</p>
            <h2>{getString(readiness, "headline", label)}</h2>
          </div>
          <span className={`pill ${tone}`}>{label}</span>
        </div>
        <div className="route-summary-grid">
          {branch && <div><span>Branch</span><strong>{branch}</strong></div>}
          <div><span>Changed files</span><strong>{formatNumber(changed)}</strong></div>
        </div>
        {(blockers.length > 0 || warnings.length > 0) && (
          <div className="route-detail-grid">
            {blockers.length > 0 && <RouteList title="Blockers" tone="danger" items={blockers} />}
            {warnings.length > 0 && <RouteList title="Warnings" tone="warn" items={warnings} />}
          </div>
        )}
        {canCommit && (
          <div className="commit-box">
            <input
              value={commitMessage}
              onChange={(e) => setCommitMessage(e.target.value)}
              placeholder="Commit message"
              disabled={isCommitting}
            />
            {!confirmCommit ? (
              <button className="primary-button" disabled={!commitMessage.trim() || isCommitting} onClick={() => setConfirmCommit(true)}>
                Commit {changed} file{changed === 1 ? "" : "s"}…
              </button>
            ) : (
              <div className="button-row">
                <button className="primary-button" disabled={isCommitting} onClick={() => void runCommit()}>
                  {isCommitting ? "Committing…" : `Confirm commit to ${branch || "branch"}`}
                </button>
                <button className="ghost-button" disabled={isCommitting} onClick={() => setConfirmCommit(false)}>Cancel</button>
              </div>
            )}
            {warnings.length > 0 && <p className="muted">Committing with {warnings.length} warning(s) — review above first.</p>}
            {commitError && <div className="notice danger">{commitError.message}</div>}
          </div>
        )}
      </section>
    );
  }

  function renderChangedFiles() {
    const files = asArray(getValue(git, "changed_files"));
    return (
      <section className="panel">
        <p className="eyebrow">What changed</p>
        {files.length === 0 ? (
          <p className="muted">Working tree is clean.</p>
        ) : (
          <ul className="changed-files">
            {files.slice(0, 12).map((file, index) => {
              const record = asRecord(file);
              const path = getString(record, "path", "");
              const isStaged = Boolean(record.staged);
              return (
                <li key={getString(record, "path", String(index))}>
                  <button 
                    className="ghost-button" 
                    style={{ textAlign: "left", padding: "0.25rem 0.5rem", width: "100%", justifyContent: "flex-start", fontFamily: "monospace" }}
                    onClick={() => setDiffFile({ path, cached: isStaged })}
                    title={`View diff for ${path}`}
                  >
                    <code>{getString(record, "status_code", "")}</code> {path}
                  </button>
                </li>
              );
            })}
            {files.length > 12 && <li className="muted" style={{ padding: "0.5rem" }}>+{files.length - 12} more</li>}
          </ul>
        )}
      </section>
    );
  }

  function renderBestRoutePanel() {
    const decision = routing?.decision;
    const request = routing?.request;
    const recommendedId = decision?.recommended_executor_id ?? decision?.recommended_provider;
    const recommended = decision?.candidates?.find((candidate: any) =>
      candidate.executor_id === recommendedId || candidate.provider === decision.recommended_provider
    );
    const candidateRows = decision?.candidates?.slice(0, 5) ?? [];

    return (
      <section className="panel route-panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Best route</p>
            <h2>{decision ? `${recommendedId}${decision.recommended_model ? ` / ${decision.recommended_model}` : ""}` : "No route loaded"}</h2>
          </div>
          <span className={`pill ${statusTone(decision?.decision_level)}`}>{decision?.decision_level ?? "unknown"}</span>
        </div>

        {!decision ? (
          <p className="muted">Refresh workspace to calculate a route from current tokens, models, budget, and Git state.</p>
        ) : (
          <>
            <div className="route-summary-grid">
              <div><span>Task</span><strong>{request?.task_kind ?? decision.task_kind}</strong></div>
              <div><span>Score</span><strong>{decision.score}</strong></div>
              <div><span>Tokens</span><strong>{formatNumber(decision.estimated_total_tokens)}</strong></div>
              <div><span>Cost</span><strong>{formatCost(recommended?.estimated_cost_units, tokens?.cost_estimate.currency_label)}</strong></div>
              <div><span>Fallback</span><strong>{decision.fallback_executor_id ?? decision.fallback_provider ?? "manual"}</strong></div>
              <div><span>Files</span><strong>{formatNumber(request?.changed_file_count)}</strong></div>
            </div>

            {(decision.blockers.length > 0 || decision.warnings.length > 0 || decision.required_guardrails.length > 0) && (
              <div className="route-detail-grid">
                {decision.blockers.length > 0 && <RouteList title="Blockers" tone="danger" items={decision.blockers} />}
                {decision.warnings.length > 0 && <RouteList title="Warnings" tone="warn" items={decision.warnings} />}
                {decision.required_guardrails.length > 0 && <RouteList title="Guardrails" tone="ok" items={decision.required_guardrails} />}
              </div>
            )}

            <div className="route-candidate-strip">
              {candidateRows.map((candidate: any) => (
                <div className={`route-candidate ${candidate.blocked ? "blocked" : ""}`} key={candidate.provider}>
                  <strong>{candidate.provider}</strong>
                  <span>{candidate.kind} - {candidate.blocked ? "blocked" : `score ${candidate.score}`}</span>
                </div>
              ))}
            </div>
          </>
        )}
      </section>
    );
  }

  return (
    <div className="content-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Workflow</p>
        <h1>{getString(workflow, "primary_cta", nextAction?.label ?? "One safe next step")}</h1>
        <p className="lead">{nextAction?.description ?? "Connect a project and task, then build bounded context before model usage."}</p>
        <div className="button-row">
          <button className="primary-button" onClick={() => void doNextSafeStep()} disabled={isBusy || needsOnboarding}>{nextAction?.label ?? "Do next safe step"}</button>
          <button className="ghost-button" onClick={() => void queryClient.invalidateQueries()} disabled={isBusy}>Refresh workflow</button>
        </div>
      </section>

      {needsOnboarding ? renderOnboarding() : (
        <>
          {diffFile && <DiffViewerModal filePath={diffFile.path} cached={diffFile.cached} onClose={() => setDiffFile(null)} />}
          <TaskSwitcher />
          {renderCommitReadiness()}
          {promptsOk && <PromptsPanel />}
          {renderBestRoutePanel()}

          <section className="panel wide-panel">
            <div className="timeline">
              {steps.length === 0 ? <p className="muted">No workflow steps loaded yet.</p> : steps.map((step, index) => {
                const record = asRecord(step);
                const status = getString(record, "status", "unknown");
                return (
                  <div key={getString(record, "id", String(index))} className={`timeline-step ${statusTone(status)}`}>
                    <span>{index + 1}</span>
                    <strong>{getString(record, "title", `Step ${index + 1}`)}</strong>
                    <small>{status}</small>
                    <p>{getString(record, "description", "")}</p>
                  </div>
                );
              })}
            </div>
          </section>

          <section className="panel">
            <p className="eyebrow">Next action</p>
            <h2>{nextAction?.label ?? "No action loaded"}</h2>
            <p className="muted">{nextAction?.description ?? "Open Settings to connect project and task."}</p>
            {dirty && <div className="notice warn">Workspace has Git changes. Review them before agent-like actions.</div>}
          </section>

          {renderChangedFiles()}

          <section className="panel">
            <p className="eyebrow">Last result</p>
            <pre className="code-panel compact">{lastResult ? stringifyPreview(lastResult, 4000) : "No action has run in this session."}</pre>
          </section>
        </>
      )}
    </div>
  );
}
