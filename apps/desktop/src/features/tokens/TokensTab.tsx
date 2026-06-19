import React from "react";
import { useState } from "react";
import { formatNumber, formatCost, statusTone, MetricCard, UsageRows, Sparkline, EmptyState } from "../../shared/ui/SharedComponents";
import { useTokens } from "./useTokens";
import { useWorkspace } from "../../shared/hooks/useWorkspace";

interface TokensTabProps {}

export function TokensTab({}: TokensTabProps) {
  const { tokens, fileTokenEstimates, costTrend, loadEstimates, logTokenUsage, saveIgnoreRules } = useTokens();
  const { projectConfig } = useWorkspace();
  const [tokenLogForm, setTokenLogForm] = useState({ provider: "manual", model: "", inputTokens: "0", outputTokens: "0", category: "general", notes: "" });
  const isBusy = false;
  const refreshAll = () => {};

  const handleToggleIgnore = async (path: string) => {
    if (!projectConfig) return;
    const current = projectConfig.context_ignore || [];
    const nextIgnore = current.includes(path) ? current.filter((item: string) => item !== path) : [...current, path];
    await saveIgnoreRules(nextIgnore);
  };

  const handleRemoveIgnoreRule = async (rule: string) => {
    if (!projectConfig) return;
    const nextIgnore = (projectConfig.context_ignore || []).filter((item: string) => item !== rule);
    await saveIgnoreRules(nextIgnore);
  };

  const providerRows = tokens?.by_provider ?? [];
  const modelRows = tokens?.by_model ?? [];

  return (
    <div className="content-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Tokens</p>
        <h1>{formatNumber(tokens?.totals.total_tokens)} total tokens logged.</h1>
        <p className="lead">Track active artifact estimates, manual usage, and planning cost before sending context to local or paid models.</p>
        <div className="button-row">
          <button className="ghost-button" onClick={() => void refreshAll()} disabled={isBusy}>Refresh tokens</button>
        </div>
      </section>

      <div className="card-row">
        <MetricCard label="Input" value={formatNumber(tokens?.totals?.total_input_tokens)} detail="Logged input tokens" />
        <MetricCard label="Output" value={formatNumber(tokens?.totals?.total_output_tokens)} detail="Logged output tokens" />
        <MetricCard label="Entries" value={formatNumber(tokens?.totals?.entries_count)} detail="Ledger rows" />
        <MetricCard label="Estimated cost" value={formatCost(tokens?.cost_estimate?.estimated_total_units, tokens?.cost_estimate?.currency_label)} detail="Local planning units" />
      </div>

      {tokens?.totals && (() => {
        const todayUsed = tokens.totals.today_total_tokens ?? 0;
        const remaining = tokens.totals.remaining_daily_tokens ?? 0;
        const hardLimit = todayUsed + remaining;
        const pctUsed = hardLimit > 0 ? Math.min(100, (todayUsed / hardLimit) * 100) : 0;
        return (
        <section className="panel wide-panel flex-col gap-lg">
          <div className="panel-title-row flex justify-between items-center">
            <div>
              <p className="eyebrow m-0">Daily Token Budget</p>
              <h2 className="mt-xs text-xl" style={{ margin: "4px 0 0 0" }}>Remaining Daily Budget</h2>
            </div>
            <span className="pill ok text-base font-bold">
              {formatNumber(remaining)} tokens left
            </span>
          </div>

          <div className="flex-col gap-sm">
            <div className="flex justify-between font-medium text-base">
              <span>Today's Usage: <strong>{formatNumber(todayUsed)}</strong> tokens</span>
              <span className="text-muted">Hard Limit: {formatNumber(hardLimit)} tokens</span>
            </div>

            <div className="progress-track">
              <div className="progress-fill" style={{ width: `${pctUsed}%` }} />
            </div>

            <div className="flex justify-between text-sm mt-xs text-muted">
              <span>{pctUsed.toFixed(1)}% used</span>
              <span>Resetting daily (UTC)</span>
            </div>
          </div>
        </section>
        );
      })()}

      {costTrend.length > 0 && (
        <section className="panel wide-panel">
          <div className="panel-title-row">
            <div>
              <p className="eyebrow">Cost trend</p>
              <h2>Last {costTrend.length} days</h2>
            </div>
            <span className="pill neutral">
              {formatCost(costTrend.reduce((sum, p) => sum + p.cost_units, 0), tokens?.cost_estimate.currency_label)} total
            </span>
          </div>
          <Sparkline values={costTrend.map((p) => p.cost_units)} width={320} height={48} label="Daily cost units" />
          <div className="table-list" style={{ marginTop: 8 }}>
            {costTrend
              .filter((p) => p.total_tokens > 0)
              .slice(-7)
              .reverse()
              .map((p) => (
                <div className="table-row" key={p.date}>
                  <span>{p.date}</span>
                  <div className="row-meta">
                    <span>{formatNumber(p.total_tokens)} tokens</span>
                    <strong>{formatCost(p.cost_units, tokens?.cost_estimate.currency_label)}</strong>
                  </div>
                </div>
              ))}
            {costTrend.every((p) => p.total_tokens === 0) && (
              <p className="muted" style={{ margin: 0 }}>No usage logged in this window yet.</p>
            )}
          </div>
        </section>
      )}

      <section className="panel wide-panel">
        <div className="panel-title-row"><div><p className="eyebrow">Active artifacts</p><h2>Context token estimates</h2></div></div>
        <div className="table-list">
          {(tokens?.active_artifacts ?? []).map((artifact: any) => (
            <div className="table-row" key={artifact.kind}>
              <div>
                <strong>{artifact.title}</strong>
                <span>{artifact.path || artifact.recommendation}</span>
              </div>
              <div className="row-meta">
                <span className={`pill ${statusTone(artifact.status)}`}>{artifact.status}</span>
                <strong>{artifact.exists ? formatNumber(artifact.estimated_tokens) : "-"}</strong>
              </div>
            </div>
          ))}
          {(tokens?.active_artifacts ?? []).length === 0 && <EmptyState message="No active task artifacts yet." />}
        </div>
      </section>

      <section className="panel">
        <p className="eyebrow">By provider</p>
        <UsageRows rows={providerRows} empty="No provider usage logged yet." />
      </section>

      <section className="panel">
        <p className="eyebrow">By model</p>
        <UsageRows rows={modelRows} empty="No model-specific usage logged yet." />
      </section>

      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Token Leak Advisor</p>
            <h2>Workspace token weights</h2>
          </div>
          <button className="tiny-button" onClick={() => void loadEstimates()}>Scan files</button>
        </div>
        <p className="text-muted mb-md">
          Identify files consuming the most tokens in your project. Click <strong>Ignore</strong> to exclude them from context packs and prevent token leaks.
        </p>
        <div className="table-list p-xs" style={{ maxHeight: "350px", overflowY: "auto", border: "1px solid var(--border)", borderRadius: "8px" }}>
          {fileTokenEstimates.map((file) => {
            const isIgnored = projectConfig?.context_ignore?.includes(file.path) || false;
            return (
              <div className="table-row" key={file.path}>
                <div className="flex-col gap-xs">
                  <strong><code>{file.path}</code></strong>
                  <span className="text-muted">{formatNumber(file.bytes)} bytes &bull; {file.status}</span>
                </div>
                <div className="row-meta">
                  <strong>{formatNumber(file.estimated_tokens)} tokens</strong>
                  <button 
                    className={`tiny-button ${isIgnored ? "active" : ""}`}
                    onClick={() => void handleToggleIgnore(file.path)}
                  >
                    {isIgnored ? "Ignored" : "Ignore"}
                  </button>
                </div>
              </div>
            );
          })}
          {fileTokenEstimates.length === 0 && <p className="text-muted p-sm text-center">No scanned text files. Click scan to analyze.</p>}
        </div>
      </section>

      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Context ignore list</p>
            <h2>Active ignore rules</h2>
          </div>
        </div>
        <p className="text-muted mb-md">
          These patterns are excluded from context compilation. Click <strong className="text-danger">&times;</strong> to remove a rule.
        </p>
        <div className="flex flex-wrap gap-sm">
          {projectConfig?.context_ignore?.map((rule: string) => (
            <span key={rule} className="pill flex items-center gap-sm">
              <code>{rule}</code>
              <button 
                className="text-danger font-bold text-base p-0"
                style={{ border: "none", background: "none", cursor: "pointer" }}
                onClick={() => void handleRemoveIgnoreRule(rule)}
              >
                &times;
              </button>
            </span>
          ))}
          {(!projectConfig?.context_ignore || projectConfig.context_ignore.length === 0) && <p className="text-muted">No ignore rules configured.</p>}
        </div>
      </section>

      <section className="panel wide-panel">
        <div className="panel-title-row"><div><p className="eyebrow">Manual log</p><h2>Add token usage</h2></div><button className="primary-button" onClick={() => void logTokenUsage(tokenLogForm)} disabled={isBusy}>Log usage</button></div>
        <div className="form-grid">
          <label>Provider<input value={tokenLogForm.provider} onChange={(event) => setTokenLogForm({ ...tokenLogForm, provider: event.target.value })} /></label>
          <label>Model<input value={tokenLogForm.model} onChange={(event) => setTokenLogForm({ ...tokenLogForm, model: event.target.value })} placeholder="optional" /></label>
          <label>Input tokens<input inputMode="numeric" value={tokenLogForm.inputTokens} onChange={(event) => setTokenLogForm({ ...tokenLogForm, inputTokens: event.target.value })} /></label>
          <label>Output tokens<input inputMode="numeric" value={tokenLogForm.outputTokens} onChange={(event) => setTokenLogForm({ ...tokenLogForm, outputTokens: event.target.value })} /></label>
          <label>Category<input value={tokenLogForm.category} onChange={(event) => setTokenLogForm({ ...tokenLogForm, category: event.target.value })} /></label>
          <label className="span-2">Notes<textarea rows={3} value={tokenLogForm.notes} onChange={(event) => setTokenLogForm({ ...tokenLogForm, notes: event.target.value })} /></label>
        </div>
      </section>
    </div>
  );
}
