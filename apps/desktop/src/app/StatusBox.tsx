export function StatusBox({
  label,
  value,
  ok,
  hint,
  onClick,
}: {
  label: string;
  value: string;
  ok: boolean;
  hint?: string;
  onClick?: () => void;
}) {
  const className = `status-box ${ok ? "ok" : "warn"}${onClick ? " clickable" : ""}`;
  const body = (
    <>
      <span className={`status-dot ${ok ? "ok" : "warn"}`} />
      <span>{label}</span>
      <strong>{value}</strong>
    </>
  );
  if (onClick) {
    return (
      <button type="button" className={className} onClick={onClick} title={hint ?? `Open ${label}`}>
        {body}
      </button>
    );
  }
  return (
    <div className={className} title={hint}>
      {body}
    </div>
  );
}
