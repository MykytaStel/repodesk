import type { RecoveryState } from "../../shared/api/recovery";
import { useRecovery } from "./RecoveryProvider";

const STATE_LABELS: Record<RecoveryState, string> = {
  healthy: "Healthy",
  degraded: "Degraded",
  repairing: "Repairing",
  needs_approval: "Needs approval",
  blocked: "Blocked",
  unknown: "Unknown",
};

export function IDEHealthPanel() {
  const { panelOpen, snapshot, selected, mutationProgress, closeHealth, openHealth } = useRecovery();
  if (!panelOpen) return null;

  return (
    <div className="ide-health-overlay" onMouseDown={(event) => {
      if (event.target === event.currentTarget) closeHealth();
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
          <button type="button" className="ide-health-close" onClick={closeHealth} aria-label="Close IDE Health">
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
                  {record.title}
                </button>
              ))}
            </nav>
          ) : null}

          {selected ? (
            <article className="ide-health-record">
              <div className="ide-health-record-heading">
                <span className={`ide-health-state ${selected.state}`}>
                  {STATE_LABELS[selected.state]}
                </span>
                <span>{selected.module_id.replace(/_/g, " ")}</span>
              </div>
              <h3>{selected.title}</h3>
              <p>{selected.explanation}</p>
            </article>
          ) : (
            <div className="ide-health-empty">
              <h3>No IDE problems detected</h3>
              <p>RepoDesk will show actionable workspace diagnostics here.</p>
            </div>
          )}

          <div className="ide-health-progress" aria-live="polite" aria-atomic="true">
            {mutationProgress ?? ""}
          </div>
        </div>
      </section>
    </div>
  );
}
