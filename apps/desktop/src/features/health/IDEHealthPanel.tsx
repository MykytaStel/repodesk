import { useEffect, useMemo } from "react";
import type {
  RecoveryAction,
  RecoveryAttempt,
  RecoveryRisk,
  RecoveryState,
} from "../../shared/api/recovery";
import { useRecovery } from "./RecoveryProvider";
import "./health-panel.css";

const STATE_LABELS: Record<RecoveryState, string> = {
  healthy: "Healthy",
  degraded: "Degraded",
  repairing: "Repairing",
  needs_approval: "Needs approval",
  blocked: "Blocked",
  unknown: "Unknown",
};

const RISK_LABELS: Record<RecoveryRisk, string> = {
  low: "Low risk",
  moderate: "Moderate risk",
  high: "High risk",
};

function attemptLabel(attempt: RecoveryAttempt): string {
  if (!attempt.result) return "Running";
  if (attempt.result === "verified") return "Verified";
  if (attempt.result === "verification_failed") return "Verification failed";
  if (attempt.result === "cancelled") return "Cancelled";
  return "Failed";
}

function RecoveryActionButton({
  action,
  disabled,
  onAutomatic,
  onConfirmable,
}: {
  action: RecoveryAction;
  disabled: boolean;
  onAutomatic: () => void;
  onConfirmable: () => void;
}) {
  if (action.kind === "manual") {
    return (
      <div className="ide-health-manual-action">
        <strong>{action.label}</strong>
        <span>Manual action required. RepoDesk will not run this step automatically.</span>
      </div>
    );
  }

  return (
    <button
      type="button"
      className={action.kind === "confirmable" ? "ide-health-primary-action" : "ide-health-secondary-action"}
      disabled={disabled}
      onClick={action.kind === "confirmable" ? onConfirmable : onAutomatic}
    >
      {action.label}
    </button>
  );
}

export function IDEHealthPanel() {
  const {
    panelOpen,
    snapshot,
    history,
    selected,
    previewState,
    mutationProgress,
    mutationError,
    closeHealth,
    openHealth,
    dismissPreview,
    check,
    preview,
    confirm,
    cancel,
  } = useRecovery();
  const selectedHistory = useMemo(
    () => history.filter((attempt) => attempt.capability_id === selected?.capability_id).slice(0, 4),
    [history, selected?.capability_id],
  );
  const busy = mutationProgress !== null;
  const repairing = mutationProgress === "Repairing";

  useEffect(() => {
    if (!panelOpen) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (previewState && !repairing) dismissPreview();
      else closeHealth();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [closeHealth, dismissPreview, panelOpen, previewState, repairing]);

  if (!panelOpen) return null;

  return (
    <div className="ide-health-overlay" onMouseDown={(event) => {
      if (event.target === event.currentTarget && !repairing) closeHealth();
    }}>
      <section
        className="ide-health-panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby="ide-health-title"
      >
        <header className="ide-health-panel-header">
          <div>
            <span className="ide-health-eyebrow">Workspace diagnostics</span>
            <h2 id="ide-health-title">IDE Health</h2>
          </div>
          <button
            type="button"
            className="ide-health-close"
            onClick={closeHealth}
            aria-label="Close IDE Health"
          >
            ×
          </button>
        </header>

        <div className="ide-health-panel-body">
          {snapshot?.warnings.map((warning) => (
            <p className="ide-health-warning" key={warning}>{warning}</p>
          ))}

          {snapshot && snapshot.records.length > 1 ? (
            <nav className="ide-health-record-nav" aria-label="IDE health capabilities">
              {snapshot.records.map((record) => (
                <button
                  type="button"
                  key={record.capability_id}
                  className={record.capability_id === selected?.capability_id ? "active" : ""}
                  onClick={() => openHealth(record.capability_id)}
                >
                  <span className={`ide-health-nav-dot ${record.state}`} aria-hidden="true" />
                  {record.title}
                </button>
              ))}
            </nav>
          ) : null}

          {selected ? (
            <>
              <article className="ide-health-record">
                <div className="ide-health-record-heading">
                  <span className={`ide-health-state ${selected.state}`}>
                    {STATE_LABELS[selected.state]}
                  </span>
                  <span>{selected.module_id.replace(/_/g, " ")}</span>
                </div>
                <div className="ide-health-title-row">
                  <div>
                    <h3>{selected.title}</h3>
                    <p>{selected.explanation}</p>
                  </div>
                  <button
                    type="button"
                    className="ide-health-secondary-action"
                    disabled={busy}
                    onClick={() => void check(selected.capability_id).catch(() => undefined)}
                  >
                    Re-check
                  </button>
                </div>

                {selected.evidence.length > 0 ? (
                  <dl className="ide-health-evidence">
                    {selected.evidence.map((item) => (
                      <div key={`${item.label}:${item.value}`}>
                        <dt>{item.label}</dt>
                        <dd>{item.value}</dd>
                      </div>
                    ))}
                  </dl>
                ) : null}

                <div className="ide-health-impact-grid">
                  <section>
                    <h4>Affected</h4>
                    <div className="ide-health-chip-list">
                      {selected.affected.length > 0
                        ? selected.affected.map((item) => <span className="affected" key={item}>{item}</span>)
                        : <span className="quiet">Nothing detected</span>}
                    </div>
                  </section>
                  <section>
                    <h4>Still available</h4>
                    <div className="ide-health-chip-list">
                      {selected.unaffected.map((item) => <span className="available" key={item}>{item}</span>)}
                    </div>
                  </section>
                </div>

                {selected.actions.length > 0 ? (
                  <section className="ide-health-actions" aria-label="Recovery actions">
                    <h4>Recovery</h4>
                    <div className="ide-health-action-list">
                      {selected.actions.map((action) => (
                        <RecoveryActionButton
                          key={action.id}
                          action={action}
                          disabled={busy}
                          onAutomatic={() => void check(selected.capability_id).catch(() => undefined)}
                          onConfirmable={() => void preview(selected.capability_id, action.id).catch(() => undefined)}
                        />
                      ))}
                    </div>
                    {selected.automatic_attempts > 0 ? (
                      <p className="ide-health-attempt-note">
                        RepoDesk already attempted {selected.automatic_attempts} automatic restart{selected.automatic_attempts === 1 ? "" : "s"} for this diagnosis.
                      </p>
                    ) : null}
                  </section>
                ) : null}
              </article>

              {previewState ? (
                <section className="ide-health-preview" aria-labelledby="ide-health-preview-title">
                  <div className="ide-health-preview-heading">
                    <div>
                      <span className={`ide-health-risk ${previewState.risk}`}>
                        {RISK_LABELS[previewState.risk]}
                      </span>
                      <h3 id="ide-health-preview-title">{previewState.title}</h3>
                    </div>
                    {!repairing ? (
                      <button type="button" className="ide-health-text-action" onClick={dismissPreview}>
                        Dismiss
                      </button>
                    ) : null}
                  </div>
                  <p>{previewState.summary}</p>

                  <div className="ide-health-preview-facts">
                    <span><strong>Network</strong>{previewState.network_required ? "Required" : "Not required"}</span>
                    <span><strong>Verification</strong>{previewState.verification}</span>
                    <span>
                      <strong>Approval expires</strong>
                      {new Date(previewState.expires_at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
                    </span>
                  </div>

                  <div className="ide-health-change-list">
                    <h4>Planned changes</h4>
                    <ul>
                      {previewState.changes.map((change) => <li key={change}>{change}</li>)}
                    </ul>
                  </div>

                  <div className="ide-health-preview-actions">
                    {repairing ? (
                      <button
                        type="button"
                        className="ide-health-danger-action"
                        onClick={() => void cancel(previewState.recipe_id).catch(() => undefined)}
                      >
                        Cancel repair
                      </button>
                    ) : (
                      <>
                        <button type="button" className="ide-health-secondary-action" onClick={dismissPreview}>
                          Cancel
                        </button>
                        <button
                          type="button"
                          className="ide-health-primary-action"
                          disabled={busy}
                          onClick={() => void confirm(previewState.confirmation_token).catch(() => undefined)}
                        >
                          Approve repair
                        </button>
                      </>
                    )}
                  </div>
                </section>
              ) : null}

              {selectedHistory.length > 0 ? (
                <section className="ide-health-history">
                  <h4>Recent recovery attempts</h4>
                  {selectedHistory.map((attempt) => (
                    <div className="ide-health-history-row" key={attempt.id}>
                      <span className={`ide-health-history-result ${attempt.result ?? "running"}`}>
                        {attemptLabel(attempt)}
                      </span>
                      <div>
                        <strong>{attempt.action_id.replace(/-/g, " ")}</strong>
                        {attempt.verification_summary ? <p>{attempt.verification_summary}</p> : null}
                      </div>
                    </div>
                  ))}
                </section>
              ) : null}
            </>
          ) : (
            <div className="ide-health-empty">
              <h3>No IDE problems detected</h3>
              <p>RepoDesk will show actionable workspace diagnostics here.</p>
            </div>
          )}

          {mutationError ? (
            <div className="ide-health-error" role="alert">{mutationError}</div>
          ) : null}
          <div className="ide-health-progress" aria-live="polite" aria-atomic="true">
            {mutationProgress ?? ""}
          </div>
        </div>
      </section>
    </div>
  );
}
