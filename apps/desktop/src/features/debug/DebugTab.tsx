import React, { useState } from "react";
import { stringifyPreview } from "../../shared/ui/SharedComponents";
import { callCommand } from "../../shared/api/queries";
import { getRuntimeMetricsSnapshot, resetRuntimeMetrics } from "../../shared/api/runtimeMetrics";

import { useDebug } from "./useDebug";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { useWorkflow } from "../workflow/useWorkflow";
import { useGit } from "../git/useGit";
import { useTokens } from "../tokens/useTokens";
import { useModels } from "../models/useModels";
import { useSettings } from "../settings/useSettings";
import "./debug-route.css";
import "../routing/routing-feature.css";

export function DebugTab() {
  const { debugEvents, artifactKind, artifactContent, requestArtifact, pendingPaid, confirmPaidReveal, cancelPaidReveal } = useDebug();
  const { snapshot, dbState } = useWorkspace({ includeDbStatus: true });
  const { workflow, history } = useWorkflow();
  const { git } = useGit();
  const { tokens } = useTokens();
  const { models } = useModels();
  const { providerSettings } = useSettings();

  const [backupMsg, setBackupMsg] = useState("");
  const [restorePath, setRestorePath] = useState("");
  const [dataBusy, setDataBusy] = useState(false);
  const [, setMetricEpoch] = useState(0);
  const runtime = getRuntimeMetricsSnapshot();

  async function runBackup() {
    setDataBusy(true);
    try {
      const path = await callCommand<string>("backup_state");
      setBackupMsg(`Backed up to ${path}`);
    } catch (error: any) {
      setBackupMsg(error?.message || String(error));
    } finally {
      setDataBusy(false);
    }
  }

  async function runRestore() {
    setDataBusy(true);
    try {
      const result = await callCommand<string>("restore_state", { path: restorePath });
      setBackupMsg(result);
    } catch (error: any) {
      setBackupMsg(error?.message || String(error));
    } finally {
      setDataBusy(false);
    }
  }

  return (
    <div className="content-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Debug</p>
        <h1>{debugEvents.length} command traces.</h1>
        <p className="lead">Raw state and bounded runtime telemetry live here so product surfaces stay focused.</p>
      </section>

      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Runtime</p>
            <h2>Instrumented IPC cost</h2>
          </div>
          <button
            className="tiny-button"
            onClick={() => {
              resetRuntimeMetrics();
              setMetricEpoch((value) => value + 1);
            }}
          >
            Reset
          </button>
        </div>
        <p className="muted">Covers commands using the shared callCommand transport. Typed direct-invoke APIs will migrate to the same measured transport incrementally.</p>
        <div className="route-summary-grid">
          <div><span>Calls</span><strong>{runtime.total_calls}</strong></div>
          <div><span>Errors</span><strong>{runtime.total_errors}</strong></div>
          <div><span>Total time</span><strong>{runtime.total_ms.toLocaleString()}ms</strong></div>
          <div><span>Commands</span><strong>{runtime.tracked_commands}</strong></div>
        </div>
        <div className="debug-runtime-table" role="table" aria-label="Instrumented IPC runtime metrics">
          <div className="debug-runtime-row header" role="row">
            <span>Command</span><span>Calls</span><span>Total</span><span>Max</span><span>Errors</span>
          </div>
          {runtime.commands.slice(0, 12).map((metric) => (
            <div className="debug-runtime-row" role="row" key={metric.command}>
              <code>{metric.command}</code>
              <span>{metric.calls}</span>
              <span>{metric.total_ms.toLocaleString()}ms</span>
              <span>{metric.max_ms.toLocaleString()}ms</span>
              <span>{metric.errors}</span>
            </div>
          ))}
          {runtime.commands.length === 0 ? <p className="muted">No instrumented IPC calls recorded yet.</p> : null}
        </div>
      </section>

      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div><p className="eyebrow">Artifacts</p><h2>Prompt and context viewer</h2></div>
          <button className="tiny-button" onClick={() => void requestArtifact(artifactKind)}>Load</button>
        </div>
        <div className="button-row compact-buttons">
          {["context", "smart_context", "prompt_codex", "prompt_chatgpt", "prompt_review", "checks_summary", "token_estimate"].map((kind) => (
            <button key={kind} className={artifactKind === kind ? "tiny-button active" : "tiny-button"} onClick={() => void requestArtifact(kind)}>{kind}</button>
          ))}
        </div>
        {pendingPaid ? (
          <div className={`notice ${pendingPaid.gate.decision === "BLOCK" ? "danger" : "warn"}`}>
            <strong>Paid/cloud agent hand-off: {pendingPaid.gate.agent}</strong>
            <p>RepoDesk will not send anything automatically. Safety judgement: <strong>{pendingPaid.gate.decision}</strong>. Review before copying this prompt into an external tool.</p>
            {pendingPaid.gate.reasons.length > 0 && (
              <ul className="compact-list">
                {pendingPaid.gate.reasons.slice(0, 4).map((reason) => <li key={reason}>{reason}</li>)}
              </ul>
            )}
            <div className="button-row">
              <button className="primary-button" onClick={() => void confirmPaidReveal()}>Reveal prompt anyway</button>
              <button className="ghost-button" onClick={() => cancelPaidReveal()}>Cancel</button>
            </div>
          </div>
        ) : (
          <pre className="code-panel tall">{artifactContent || "Choose an artifact to preview."}</pre>
        )}
      </section>

      <section className="panel wide-panel">
        <p className="eyebrow">Command traces</p>
        <div className="debug-list">
          {debugEvents.map((event) => (
            <details key={event.id} className={`debug-event ${event.status}`}>
              <summary><strong>{event.command}</strong><span>{event.status}</span><small>{event.durationMs}ms - {event.timestamp}</small></summary>
              <pre>{event.error ?? event.preview ?? "No output."}</pre>
            </details>
          ))}
        </div>
      </section>

      <section className="panel wide-panel">
        <p className="eyebrow">Local data</p>
        <h2>Backup &amp; restore</h2>
        <p className="muted">Action history, memory, events, and token ledger live in one local SQLite database.</p>
        <div className="button-row">
          <button className="primary-button" disabled={dataBusy} onClick={() => void runBackup()}>Back up now</button>
        </div>
        <div className="form-grid">
          <label>Restore from path<input value={restorePath} onChange={(e) => setRestorePath(e.target.value)} placeholder="/path/to/repodesk-….sqlite" /></label>
          <button className="ghost-button" disabled={dataBusy || !restorePath.trim()} onClick={() => void runRestore()}>Restore</button>
        </div>
        {backupMsg && <div className="notice">{backupMsg}</div>}
      </section>

      <section className="panel wide-panel"><p className="eyebrow">Action history</p><pre className="code-panel tall">{(history && history.length) ? stringifyPreview(history, 8000) : "No action history yet."}</pre></section>
      <section className="panel wide-panel"><p className="eyebrow">Raw state</p><pre className="code-panel tall">{stringifyPreview({ snapshot, workflow, git, tokens, models, providerSettings, dbState }, 14000)}</pre></section>
    </div>
  );
}
