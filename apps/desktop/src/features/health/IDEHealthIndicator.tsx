import { useRecovery } from "./RecoveryProvider";

const ACTIONABLE_STATES = new Set(["degraded", "needs_approval", "blocked"]);

export function IDEHealthIndicator() {
  const { snapshot, openHealth } = useRecovery();
  const attentionCount =
    snapshot?.records.filter((record) => ACTIONABLE_STATES.has(record.state)).length ?? 0;
  const label = attentionCount === 0
    ? "IDE health: Healthy"
    : `IDE health: ${attentionCount} ${attentionCount === 1 ? "needs" : "need"} attention`;

  return (
    <button
      type="button"
      className={`ide-health-indicator${attentionCount > 0 ? " needs-attention" : ""}`}
      aria-label={label}
      onClick={() => openHealth()}
    >
      <span aria-hidden="true" className="ide-health-indicator-dot" />
      <span>IDE Health</span>
      {attentionCount > 0 ? <strong>{attentionCount}</strong> : null}
    </button>
  );
}
