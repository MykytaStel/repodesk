import { useQuery } from "@tanstack/react-query";
import { auditSnapshot, type CanonicalAuditEvent } from "../../shared/api/audit";
import { EmptyState, MetricCard, errorToMessage, formatNumber } from "../../shared/ui/SharedComponents";
import "./audit-route.css";
import "../routing/routing-feature.css";

const RECENT_LIMIT = 50;

type PillTone = "ok" | "warn" | "danger" | "accent" | "neutral";

function formatTimestamp(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

function formatRelative(value: string | undefined): string {
  if (!value) return "No events";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  const seconds = Math.max(0, Math.round((Date.now() - date.getTime()) / 1000));
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 48) return `${hours}h ago`;
  return `${Math.round(hours / 24)}d ago`;
}

function truncateDetail(value: string): string {
  const trimmed = value.trim();
  if (trimmed.length <= 240) return trimmed || "—";
  return `${trimmed.slice(0, 240)}…`;
}

function eventTone(level: string): PillTone {
  switch (level.toLowerCase()) {
    case "error": return "danger";
    case "security": return "danger";
    case "warn":
    case "warning": return "warn";
    case "ui": return "accent";
    default: return "neutral";
  }
}

function AuditEventRow({ event }: { event: CanonicalAuditEvent }) {
  return (
    <div className="table-row flex-col items-start gap-sm">
      <div className="w-full flex justify-between items-center gap-sm">
        <div className="flex-col gap-xs">
          <strong>{event.project || "Unbound project"}</strong>
          <span>{truncateDetail(event.message)}</span>
        </div>
        <span className={`pill ${eventTone(event.level)}`}>{event.level}</span>
      </div>
      <div className="row-meta">
        <span>{formatTimestamp(event.timestamp)}</span>
        <code>{event.module_name}</code>
        {event.task_id ? <code title={event.task_id}>Work Item · {event.task_id}</code> : null}
      </div>
    </div>
  );
}

export function AuditTab() {
  const snapshotQuery = useQuery({
    queryKey: ["runs", "canonical-ledger", RECENT_LIMIT],
    queryFn: () => auditSnapshot(RECENT_LIMIT),
  });

  const snapshot = snapshotQuery.data;
  const events = snapshot?.entries ?? [];
  const latest = events[0];
  const error = snapshotQuery.error;
  const securityCount = snapshot?.counts_by_severity.security ?? 0;
  const errorCount = snapshot?.counts_by_severity.error ?? 0;
  const warningCount = (snapshot?.counts_by_severity.warn ?? 0) + (snapshot?.counts_by_severity.warning ?? 0);

  return (
    <div className="content-grid dashboard-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Canonical engineering ledger</p>
        <h1>{error ? "Ledger integrity blocked." : snapshotQuery.isLoading ? "Verifying ledger…" : "Verified evidence projection."}</h1>
        <p className="lead">
          This view is backed by RepoDesk&apos;s canonical SQLite engineering ledger. The backend verifies
          every sequence number and hash-chain link before returning any rows; a corrupt ledger fails closed.
        </p>
        <div className="button-row">
          <button className="primary-button" onClick={() => void snapshotQuery.refetch()} disabled={snapshotQuery.isFetching}>
            {snapshotQuery.isFetching ? "Verifying…" : "Refresh evidence"}
          </button>
        </div>
      </section>

      {error ? <div className="notice danger wide-panel">{errorToMessage(error)}</div> : null}

      <div className="card-row">
        <MetricCard
          label="Ledger integrity"
          value={error ? "Blocked" : snapshotQuery.isLoading ? "Checking" : "Verified"}
          detail={error ? "No evidence is trusted until the ledger verifies." : "Full sequence/hash chain verified before projection."}
          tone={error ? "warn" : snapshotQuery.isLoading ? "neutral" : "ok"}
        />
        <MetricCard
          label="Total events"
          value={formatNumber(snapshot?.total_entries ?? 0)}
          detail={`${formatNumber(snapshot?.returned ?? 0)} newest rows retained in this view`}
        />
        <MetricCard
          label="Last event"
          value={formatRelative(latest?.timestamp)}
          detail={latest ? `${latest.module_name} · ${latest.project || "unbound"}` : "Nothing recorded yet"}
        />
      </div>

      <section className="panel wide-panel">
        <div className="panel-title-row compact">
          <div>
            <p className="eyebrow">Severity across verified ledger</p>
            <h2>{securityCount + errorCount + warningCount} attention events</h2>
          </div>
          <div className="row-meta">
            <span className="pill danger">{securityCount} security</span>
            <span className="pill danger">{errorCount} error</span>
            <span className="pill warn">{warningCount} warn</span>
          </div>
        </div>
      </section>

      <section className="panel wide-panel">
        <div className="panel-title-row compact">
          <div>
            <p className="eyebrow">Recent canonical events</p>
            <h2>Newest first</h2>
          </div>
          <span className="pill neutral">{formatNumber(events.length)}</span>
        </div>
        {snapshotQuery.isLoading ? (
          <p className="muted">Verifying and loading engineering evidence…</p>
        ) : events.length === 0 ? (
          <EmptyState
            message="No canonical engineering events recorded yet."
            hint="Work Item, execution, review, verification and UI evidence will appear here through the single SQLite ledger."
          />
        ) : (
          <div className="table-list">
            {events.map((event, index) => (
              <AuditEventRow key={`${event.timestamp}-${event.module_name}-${index}`} event={event} />
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
