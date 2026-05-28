import React from "react";
import { stringifyPreview } from "../../shared/ui/SharedComponents";

import { useDebug } from "./useDebug";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { useWorkflow } from "../workflow/useWorkflow";
import { useGit } from "../git/useGit";
import { useCode } from "../code/useCode";
import { useTokens } from "../tokens/useTokens";
import { useModels } from "../models/useModels";
import { useSettings } from "../settings/useSettings";

export function DebugTab() {
  const { debugEvents, artifactKind, artifactContent, loadArtifact } = useDebug();
  const { snapshot, dbState } = useWorkspace();
  const { workflow, history } = useWorkflow();
  const { git } = useGit();
  const { codeWorkbench } = useCode();
  const { tokens } = useTokens();
  const { models } = useModels();
  const { providerSettings } = useSettings();
  return (
    <div className="content-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Debug</p>
        <h1>{debugEvents.length} command traces.</h1>
        <p className="lead">Raw state lives here so the product screens stay focused.</p>
      </section>
      <section className="panel wide-panel">
        <div className="panel-title-row">
          <div><p className="eyebrow">Artifacts</p><h2>Prompt and context viewer</h2></div>
          <button className="tiny-button" onClick={() => void loadArtifact(artifactKind)}>Load</button>
        </div>
        <div className="button-row compact-buttons">
          {["context", "smart_context", "prompt_codex", "prompt_chatgpt", "prompt_review", "checks_summary", "token_estimate"].map((kind) => (
            <button key={kind} className={artifactKind === kind ? "tiny-button active" : "tiny-button"} onClick={() => void loadArtifact(kind)}>{kind}</button>
          ))}
        </div>
        <pre className="code-panel tall">{artifactContent || "Choose an artifact to preview."}</pre>
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
      <section className="panel wide-panel"><p className="eyebrow">Action history</p><pre className="code-panel tall">{(history && history.length) ? stringifyPreview(history, 8000) : "No action history yet."}</pre></section>
      <section className="panel wide-panel"><p className="eyebrow">Raw state</p><pre className="code-panel tall">{stringifyPreview({ snapshot, workflow, git, codeWorkbench, tokens, models, providerSettings, dbState }, 14000)}</pre></section>
    </div>
  );
}
