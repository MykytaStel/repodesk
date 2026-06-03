import React, { useState } from "react";
import { normalizeError } from "../utils/errors";
import { copyToClipboard } from "./SharedComponents";

const CATEGORY_LABEL: Record<string, string> = {
  configuration: "Setup",
  provider_transient: "Temporary",
  security_block: "Blocked",
  resource_limit: "Too large",
  internal: "Internal",
};

/**
 * Reusable error panel. Used by the error boundaries and anywhere a query or
 * mutation fails. Shows a category chip, the message, an optional Retry, and a
 * collapsible details block (stack / category / scope) with copy-to-clipboard.
 */
export function ErrorState({
  error,
  title,
  scope,
  onRetry,
  fullscreen = false,
  compact = false,
}: {
  error: unknown;
  title?: string;
  scope?: string;
  onRetry?: () => void;
  fullscreen?: boolean;
  compact?: boolean;
}) {
  const e = normalizeError(error);
  const [showDetails, setShowDetails] = useState(false);

  const details = [
    scope ? `scope: ${scope}` : null,
    `category: ${e.category}`,
    e.retryable ? "retryable: yes" : null,
    e.stack ?? null,
    e.detail ? `detail: ${typeof e.detail === "string" ? e.detail : JSON.stringify(e.detail, null, 2)}` : null,
  ]
    .filter(Boolean)
    .join("\n");

  if (compact) {
    return (
      <div className="notice warn" role="alert">
        <strong>{CATEGORY_LABEL[e.category] ?? "Error"}:</strong> {e.message}
        {onRetry && (
          <button className="tiny-button" style={{ marginLeft: 8 }} onClick={onRetry}>
            Retry
          </button>
        )}
      </div>
    );
  }

  const body = (
    <section
      className="panel wide-panel"
      role="alert"
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: fullscreen ? "center" : "flex-start",
        textAlign: fullscreen ? "center" : "left",
        padding: fullscreen ? "48px 24px" : undefined,
        gap: 10,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span className="pill danger">{CATEGORY_LABEL[e.category] ?? "Error"}</span>
        <p className="eyebrow" style={{ color: "var(--danger)", margin: 0 }}>
          {scope ?? "Error"}
        </p>
      </div>
      <h2 style={{ color: "var(--danger)", margin: 0 }}>{title ?? "Something went wrong"}</h2>
      <p className="muted" style={{ maxWidth: 560 }}>
        {e.message}
      </p>

      <div className="button-row compact-buttons" style={{ justifyContent: fullscreen ? "center" : "flex-start" }}>
        {onRetry && (
          <button className="primary-button" onClick={onRetry}>
            {e.retryable ? "Retry" : "Try again"}
          </button>
        )}
        {fullscreen && (
          <button className="tiny-button" onClick={() => window.location.reload()}>
            Reload app
          </button>
        )}
        <button className="tiny-button" onClick={() => void copyToClipboard(`${e.message}\n\n${details}`)}>
          Copy details
        </button>
        {details && (
          <button className="tiny-button" onClick={() => setShowDetails((v) => !v)}>
            {showDetails ? "Hide details" : "Details"}
          </button>
        )}
      </div>

      {showDetails && details && (
        <pre
          className="scroll-area"
          style={{ maxHeight: 220, whiteSpace: "pre-wrap", fontSize: 12, width: "100%", textAlign: "left" }}
        >
          {details}
        </pre>
      )}
    </section>
  );

  if (fullscreen) {
    return (
      <div
        style={{
          minHeight: "100vh",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          padding: 24,
        }}
      >
        <div style={{ maxWidth: 640, width: "100%" }}>{body}</div>
      </div>
    );
  }

  return <div className="content-grid">{body}</div>;
}
