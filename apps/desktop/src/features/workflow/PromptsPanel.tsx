import React, { useEffect, useState } from "react";
import { usePrompts } from "./usePrompts";
import { useTokens } from "../tokens/useTokens";
import { formatNumber, formatCost, ActorBadge } from "../../shared/ui/SharedComponents";
import { CloseIcon, ExternalLinkIcon } from "../../app/NavIcons";

function formatPrompt(content: string) {
  if (!content) return null;
  // Simple regex to highlight <tags> and **bold**
  const regex = /(<[^>]+>|\*\*[^*]+\*\*)/g;
  const parts = content.split(regex);
  return parts.map((part, i) => {
    if (part.startsWith("<") && part.endsWith(">")) {
      return <span key={i} style={{ color: "var(--accent-teal)" }}>{part}</span>;
    }
    if (part.startsWith("**") && part.endsWith("**")) {
      return <strong key={i} style={{ color: "var(--text-primary)" }}>{part.slice(2, -2)}</strong>;
    }
    return <React.Fragment key={i}>{part}</React.Fragment>;
  });
}

export function PromptsPanel() {
  const {
    artifactKind,
    artifactContent,
    isLoading,
    pendingPaid,
    requestPrompt,
    confirmPaidReveal,
    cancelPaidReveal,
    copyPrompt,
  } = usePrompts();

  const { tokens } = useTokens();
  const [isDismissed, setIsDismissed] = useState(false);

  const activeArtifacts = tokens?.active_artifacts || [];
  const currentEstimate = activeArtifacts.find((a) => a.kind === artifactKind);

  // Load the initial prompt when the panel first mounts
  useEffect(() => {
    void requestPrompt("prompt_codex", false);
  }, []);

  if (isDismissed) return null;

  return (
    <section id="generated-prompts" className="panel wide-panel prompts-panel">
      <div className="panel-title-row">
        <div>
          <p className="eyebrow">Generated Prompts</p>
          <h2>Agent hand-off</h2>
          <p className="muted" style={{ margin: "4px 0 0", fontSize: 13 }}>
            Built locally from your bounded context — no AI, no tokens, nothing sent. Copy one and paste it into the agent you trust.
          </p>
        </div>
        <div className="button-row compact-buttons">
          <ActorBadge mode="manual" />
          {!pendingPaid && artifactContent && (
            <>
              {artifactKind === "prompt_chatgpt" && (
                <a href="https://chatgpt.com/" target="_blank" rel="noreferrer" className="ghost-button" title="Open ChatGPT in browser">
                  Open ChatGPT
                  <ExternalLinkIcon />
                </a>
              )}
              <button className="primary-button" onClick={() => void copyPrompt()}>
                Copy to clipboard
              </button>
            </>
          )}
          <button
            className="ghost-button icon-button"
            onClick={() => setIsDismissed(true)}
            title="Dismiss panel"
            aria-label="Dismiss panel"
          >
            <CloseIcon />
          </button>
        </div>
      </div>

      <div className="button-row compact-buttons">
        {["agent_context_pack", "prompt_codex", "prompt_chatgpt", "prompt_review"].map((kind) => {
          const label =
            kind === "agent_context_pack" ? "Context Pack"
            : kind === "prompt_codex" ? "Codex"
            : kind === "prompt_chatgpt" ? "ChatGPT"
            : "Review / Gemini";
          return (
            <button
              key={kind}
              className={artifactKind === kind && !pendingPaid ? "tiny-button active" : "tiny-button"}
              onClick={() => void requestPrompt(kind, true)}
            >
              {label}
            </button>
          );
        })}
      </div>

      {pendingPaid ? (
        <div className={`notice ${pendingPaid.gate.decision === "BLOCK" ? "danger" : "warn"}`}>
          <strong>Paid/cloud agent hand-off: {pendingPaid.gate.agent}</strong>
          <p>
            RepoDesk will not send anything automatically. Safety judgement: <strong>{pendingPaid.gate.decision}</strong>.
            Review before copying this prompt into an external tool.
          </p>
          {pendingPaid.gate.reasons.length > 0 && (
            <ul className="compact-list">
              {pendingPaid.gate.reasons.slice(0, 4).map((reason) => (
                <li key={reason}>{reason}</li>
              ))}
            </ul>
          )}
          <div className="button-row">
            <button className="primary-button" onClick={() => void confirmPaidReveal()}>
              Reveal prompt anyway
            </button>
            <button className="ghost-button" onClick={() => cancelPaidReveal()}>
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <>
          <pre className="code-panel tall" style={{ whiteSpace: "pre-wrap", wordBreak: "break-word" }}>
            {isLoading ? "Loading prompt..." : artifactContent ? formatPrompt(artifactContent) : "Choose a prompt to view."}
          </pre>
          {currentEstimate && currentEstimate.estimated_tokens != null && !isLoading && (
            <p className="muted" style={{ marginTop: "0.5rem", fontSize: "0.85rem" }}>
              <strong>Est. tokens:</strong> {formatNumber(currentEstimate.estimated_tokens)} 
              {tokens?.cost_estimate?.currency_label && (
                <span style={{ marginLeft: "1rem" }}>
                  <strong>Est. cost:</strong> ~{formatCost((currentEstimate.estimated_tokens / 1000) * 0.005, tokens.cost_estimate.currency_label)}
                </span>
              )}
            </p>
          )}
        </>
      )}
    </section>
  );
}
