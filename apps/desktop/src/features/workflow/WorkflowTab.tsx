import React from "react";
import { asArray, asRecord, getString, stringifyPreview, formatNumber, formatCost, statusTone, RouteList } from "../../shared/ui/SharedComponents";

interface WorkflowTabProps {
  workflow: any;
  routing: any;
  tokens: any;
  nextAction: any;
  isBusy: boolean;
  dirty: boolean;
  lastResult: any;
  doNextSafeStep: () => void;
  refreshAll: (label: string) => void;
}

export function WorkflowTab({
  workflow,
  routing,
  tokens,
  nextAction,
  isBusy,
  dirty,
  lastResult,
  doNextSafeStep,
  refreshAll,
}: WorkflowTabProps) {
  const steps = asArray(getValue(workflow, "steps"));

  function getValue(source: unknown, key: string): unknown {
    return asRecord(source)[key];
  }

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
    <div className="content-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Workflow</p>
        <h1>{getString(workflow, "primary_cta", nextAction?.label ?? "One safe next step")}</h1>
        <p className="lead">{nextAction?.description ?? "Connect a project and task, then build bounded context before model usage."}</p>
        <div className="button-row">
          <button className="primary-button" onClick={() => void doNextSafeStep()} disabled={isBusy}>{nextAction?.label ?? "Do next safe step"}</button>
          <button className="ghost-button" onClick={() => void refreshAll("Refreshing workflow")} disabled={isBusy}>Refresh workflow</button>
        </div>
      </section>

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

      <section className="panel">
        <p className="eyebrow">Last result</p>
        <pre className="code-panel compact">{lastResult ? stringifyPreview(lastResult, 4000) : "No action has run in this session."}</pre>
      </section>
    </div>
  );
}
