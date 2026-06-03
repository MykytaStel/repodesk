export function StatusBox({ label, value, ok }: { label: string; value: string; ok: boolean }) {
  return (
    <div className={`status-box ${ok ? "ok" : "warn"}`}>
      <span className={`status-dot ${ok ? "ok" : "warn"}`} />
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
