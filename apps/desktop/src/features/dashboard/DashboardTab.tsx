import React, { useState } from "react";
import { formatNumber, formatCost, statusTone, MetricCard, RouteList } from "../../shared/ui/SharedComponents";
import { EconomyControl, EconomyMode } from "../routing/EconomyControl";

import { useTokens } from "../tokens/useTokens";
import { useRouting } from "../routing/useRouting";
import { useModels } from "../models/useModels";
import { useGit } from "../git/useGit";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { useWorkflow } from "../workflow/useWorkflow";
import { useQueryClient } from "@tanstack/react-query";
import { callCommand } from "../../shared/api/queries";

interface DashboardTabProps {
  setActiveTab: (tab: any) => void;
  economyMode: EconomyMode;
  setEconomyMode: (mode: any) => void;
}

export function DashboardTab({
  setActiveTab,
  economyMode,
  setEconomyMode,
}: DashboardTabProps) {
  const queryClient = useQueryClient();
  const { tokens, isLoading: isLoadingTokens } = useTokens();
  const { routing, isLoading: isLoadingRouting } = useRouting(economyMode);
  const { models, workingProviders, modelCount, isLoading: isLoadingModels } = useModels();
  const { git, branch, dirty, dirtyCount, isLoading: isLoadingGit } = useGit();
  const { projectName, taskTitle, hasProject, hasTask, isLoading: isLoadingWorkspace } = useWorkspace();
  const { nextAction, doNextSafeStep, isRunning } = useWorkflow();

  const isBusy = isRunning || isLoadingTokens || isLoadingRouting || isLoadingModels || isLoadingGit || isLoadingWorkspace;

  const [rp, setRp] = useState<any>(null);
  const [rpLoading, setRpLoading] = useState(false);
  async function runRepopilot() {
    setRpLoading(true);
    try {
      setRp(await callCommand<any>("repopilot_findings"));
    } catch (error: any) {
      setRp({ error: error?.message || String(error) });
    } finally {
      setRpLoading(false);
    }
  }

  const refreshAll = () => queryClient.invalidateQueries();
  const readiness = [
    { label: "Project", ok: hasProject, target: "settings" },
    { label: "Task", ok: hasTask, target: "settings" },
    { label: "Git", ok: Boolean(git), target: "git" },
    { label: "Tokens", ok: Boolean(tokens), target: "tokens" },
    { label: "Models", ok: Boolean(models), target: "models" },
  ];
  const readyCount = readiness.filter((item) => item.ok).length;

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
            <p className="muted" style={{ marginTop: 4, fontSize: 13 }}>Automatically determines the optimal AI model based on task complexity, codebase context size, and your cost budget.</p>
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

  function renderRepopilotPanel() {
    return (
      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">RepoPilot review</p>
            <h2>{rp && rp.health_score != null ? `Health ${rp.health_score}` : "Code review findings"}</h2>
            <p className="muted" style={{ marginTop: 4, fontSize: 13 }}>Local static analysis of your current diff. View logs in the Terminal.</p>
          </div>
          <button className="tiny-button" disabled={rpLoading} onClick={() => void runRepopilot()}>{rpLoading ? "Running (see Terminal)…" : "Run review"}</button>
        </div>
        {!rp ? (
          <p className="muted">Run a local RepoPilot review of the current diff to surface High/Critical findings before committing.</p>
        ) : rp.error ? (
          <div className="notice danger">{rp.error}</div>
        ) : (
          <>
            <div className="route-summary-grid">
              <div><span>Critical</span><strong>{rp.counts.critical}</strong></div>
              <div><span>High</span><strong>{rp.counts.high}</strong></div>
              <div><span>Medium</span><strong>{rp.counts.medium}</strong></div>
              <div><span>Low</span><strong>{rp.counts.low}</strong></div>
            </div>
            {rp.total === 0 ? (
              <p className="muted">No findings in the current diff.</p>
            ) : (
              <ul className="findings-list">
                {rp.findings.slice(0, 10).map((finding: any, index: number) => {
                  const tone = finding.severity === "CRITICAL" || finding.severity === "HIGH" ? "danger" : finding.severity === "MEDIUM" ? "warn" : "neutral";
                  return (
                    <li key={index} className="finding">
                      <span className={`pill ${tone}`}>{finding.severity}</span>
                      <strong>{finding.title}</strong>
                      {finding.file && <small>{finding.file}{finding.line ? `:${finding.line}` : ""}</small>}
                    </li>
                  );
                })}
                {rp.total > 10 && <li className="muted">+{rp.total - 10} more</li>}
              </ul>
            )}
          </>
        )}
      </section>
    );
  }



  function renderWelcomeOnboarding() {
    return (
      <section className="hero-panel wide-panel" style={{ padding: "32px" }}>
        <p className="eyebrow" style={{ marginBottom: "8px", color: "var(--accent-primary)" }}>Getting Started</p>
        <h1 style={{ fontSize: "2rem", marginBottom: "16px" }}>Welcome to RepoDesk.</h1>
        <p className="lead" style={{ fontSize: "1.1rem", marginBottom: "32px", maxWidth: "600px" }}>
          Your local-first engineering workspace. Scope one Work Item, coordinate human or agent execution, review the exact ChangeSet, and verify it before commit.
        </p>
        
        <div style={{ display: "flex", flexDirection: "column", gap: "24px" }}>
          <div style={{ display: "flex", alignItems: "flex-start", gap: "16px" }}>
            <div style={{ background: "var(--bg-card)", borderRadius: "50%", width: "32px", height: "32px", display: "flex", alignItems: "center", justifyContent: "center", fontWeight: "bold", border: "1px solid var(--border)", flexShrink: 0 }}>1</div>
            <div>
              <h3 style={{ margin: "0 0 4px 0", fontSize: "1.1rem" }}>Connect a Project</h3>
              <p className="muted" style={{ margin: 0, marginBottom: "8px", maxWidth: "500px" }}>Tell RepoDesk which local folder to manage.</p>
              <button className="secondary-cta" onClick={() => setActiveTab("settings")}>Go to Settings</button>
            </div>
          </div>
          
          <div style={{ display: "flex", alignItems: "flex-start", gap: "16px" }}>
            <div style={{ background: "var(--bg-card)", borderRadius: "50%", width: "32px", height: "32px", display: "flex", alignItems: "center", justifyContent: "center", fontWeight: "bold", border: "1px solid var(--border)", flexShrink: 0 }}>2</div>
            <div>
              <h3 style={{ margin: "0 0 4px 0", fontSize: "1.1rem" }}>Add API Keys</h3>
              <p className="muted" style={{ margin: 0, marginBottom: "8px", maxWidth: "500px" }}>Bring your own keys. They stay securely in your local OS keychain.</p>
              <button className="secondary-cta" onClick={() => setActiveTab("settings")}>Configure Providers</button>
            </div>
          </div>

          <div style={{ display: "flex", alignItems: "flex-start", gap: "16px" }}>
            <div style={{ background: "var(--bg-card)", borderRadius: "50%", width: "32px", height: "32px", display: "flex", alignItems: "center", justifyContent: "center", fontWeight: "bold", border: "1px solid var(--border)", flexShrink: 0 }}>3</div>
            <div>
              <h3 style={{ margin: "0 0 4px 0", fontSize: "1.1rem" }}>Create a Task & Let the Agent Work</h3>
              <p className="muted" style={{ margin: 0, marginBottom: "8px", maxWidth: "500px" }}>Describe what you want the AI to do. RepoDesk will build a safe, bounded context pack, run the AI, and let you review exactly what changed before committing.</p>
              <button className="primary-cta" onClick={() => setActiveTab("work")}>Open Work Flow</button>
            </div>
          </div>
        </div>
      </section>
    );
  }

  if (!hasProject && !isLoadingWorkspace) {
    return (
      <div className="content-grid dashboard-grid">
        {renderWelcomeOnboarding()}
      </div>
    );
  }

  return (
    <div className="content-grid dashboard-grid">
      {tokens && tokens.totals.total_tokens > 15000 && (
        <div className="notice warn wide-panel flex justify-between items-center w-full gap-md" style={{ gridColumn: "1 / -1" }}>
          <span>⚠️ <strong>Token Warning:</strong> Your current task context is heavy ({formatNumber(tokens.totals.total_tokens)} tokens). Review file token weights below to exclude large unnecessary files.</span>
          <button className="tiny-button" style={{ whiteSpace: "nowrap" }} onClick={() => setActiveTab("tokens")}>Open Token Advisor</button>
        </div>
      )}
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Engineering workspace</p>
        <h1>Project state, context, and verification evidence.</h1>
        <p className="lead">See the active project, next safe step, context size, available tools, and Git state before handing bounded work to an agent.</p>
        <div className="button-row">
          <button className="primary-button" onClick={() => void doNextSafeStep()} disabled={isBusy}>{nextAction ? `Do next: ${nextAction.label}` : "Do next safe step"}</button>
          <button className="ghost-button" onClick={() => void refreshAll()} disabled={isBusy}>Refresh</button>
        </div>
      </section>

      {renderBestRoutePanel()}

      {renderRepopilotPanel()}

      <EconomyControl mode={economyMode} setMode={setEconomyMode} isBusy={isBusy} />

      <section className="panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Readiness</p>
            <h2>{readyCount}/{readiness.length} ready</h2>
            <p className="muted" style={{ marginTop: 4, fontSize: 13 }}>Required setup steps before you can safely run autonomous AI agents on this project.</p>
          </div>
        </div>
        <div className="checklist-grid">
          {readiness.map((item) => (
            <button key={item.label} className={`check-card ${item.ok ? "ok" : "warn"}`} onClick={() => setActiveTab(item.target)}>
              <span>{item.ok ? "OK" : "Fix"}</span><strong>{item.label}</strong>
            </button>
          ))}
        </div>
      </section>

      <div className="card-row">
        <MetricCard label="Project" value={projectName} detail={`Task: ${taskTitle}`} />
        <MetricCard label="Git" value={dirty ? `${dirtyCount} changes` : "Clean"} detail={`Branch: ${branch}`} tone={dirty ? "warn" : "ok"} />
        <MetricCard label="Tokens" value={formatNumber(tokens?.totals.total_tokens)} detail={`${formatNumber(tokens?.totals.entries_count)} ledger entries`} />
        <MetricCard label="Models" value={`${modelCount} models`} detail={`${workingProviders} providers working`} tone={workingProviders ? "ok" : "warn"} />
      </div>
    </div>
  );
}
