import React, { useState } from "react";
import { formatNumber, formatCost, statusTone, MetricCard, RouteList } from "./SharedComponents";
import { EconomyControl, EconomyMode } from "./EconomyControl";

interface DashboardTabProps {
  tokens: any;
  routing: any;
  models: any;
  git: any;
  hasProject: boolean;
  hasTask: boolean;
  projectName: string;
  taskTitle: string;
  branch: string;
  dirty: boolean;
  dirtyCount: number;
  isBusy: boolean;
  nextAction: any;
  workingProviders: number;
  modelCount: number;
  doNextSafeStep: () => void;
  refreshAll: (label: string) => void;
  setActiveTab: (tab: any) => void;
  economyMode: string;
  setEconomyMode: (mode: any) => void;
}

export function DashboardTab({
  tokens,
  routing,
  models,
  git,
  hasProject,
  hasTask,
  projectName,
  taskTitle,
  branch,
  dirty,
  dirtyCount,
  isBusy,
  nextAction,
  workingProviders,
  modelCount,
  doNextSafeStep,
  refreshAll,
  setActiveTab,
  economyMode,
  setEconomyMode,
}: DashboardTabProps) {
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
    const recommended = decision?.candidates.find((candidate: any) => candidate.provider === decision.recommended_provider);
    const candidateRows = decision?.candidates.slice(0, 5) ?? [];

    return (
      <section className="panel route-panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Best route</p>
            <h2>{decision ? `${decision.recommended_provider}${decision.recommended_model ? ` / ${decision.recommended_model}` : ""}` : "No route loaded"}</h2>
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
              <div><span>Fallback</span><strong>{decision.fallback_provider ?? "manual"}</strong></div>
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
    <div className="content-grid dashboard-grid">
      {tokens && tokens.totals.total_tokens > 15000 && (
        <div className="notice warn wide-panel flex justify-between items-center w-full gap-md" style={{ gridColumn: "1 / -1" }}>
          <span>⚠️ <strong>Token Warning:</strong> Your current task context is heavy ({formatNumber(tokens.totals.total_tokens)} tokens). Review file token weights below to exclude large unnecessary files.</span>
          <button className="tiny-button" style={{ whiteSpace: "nowrap" }} onClick={() => setActiveTab("tokens")}>Open Token Advisor</button>
        </div>
      )}
      <section className="hero-panel wide-panel">
        <p className="eyebrow">RepoDesk cockpit</p>
        <h1>AI workflow state, tokens, and model health.</h1>
        <p className="lead">Use one screen to see the active project, next safe step, token usage, reachable models, and Git state before handing context to an agent.</p>
        <div className="button-row">
          <button className="primary-button" onClick={() => void doNextSafeStep()} disabled={isBusy}>{nextAction ? `Do next: ${nextAction.label}` : "Do next safe step"}</button>
          <button className="ghost-button" onClick={() => void refreshAll("Manual refresh")} disabled={isBusy}>Refresh</button>
        </div>
      </section>

      {renderBestRoutePanel()}
      
      <EconomyControl mode={economyMode} setMode={setEconomyMode} isBusy={isBusy} />

      <section className="panel">
        <div className="panel-title-row">
          <div><p className="eyebrow">Readiness</p><h2>{readyCount}/{readiness.length} ready</h2></div>
        </div>
        <div className="checklist-grid">
          {readiness.map((item) => (
            <button key={item.label} className={`check-card ${item.ok ? "ok" : "warn"}`} onClick={() => setActiveTab(item.target)}>
              <span>{item.ok ? "OK" : "Fix"}</span><strong>{item.label}</strong>
            </button>
          ))}
        </div>
      </section>

      <MetricCard label="Project" value={projectName} detail={`Task: ${taskTitle}`} />
      <MetricCard label="Git" value={dirty ? `${dirtyCount} changes` : "Clean"} detail={`Branch: ${branch}`} tone={dirty ? "warn" : "ok"} />
      <MetricCard label="Tokens" value={formatNumber(tokens?.totals.total_tokens)} detail={`${formatNumber(tokens?.totals.entries_count)} ledger entries`} />
      <MetricCard label="Models" value={`${modelCount} models`} detail={`${workingProviders} providers working`} tone={workingProviders ? "ok" : "warn"} />
    </div>
  );
}
