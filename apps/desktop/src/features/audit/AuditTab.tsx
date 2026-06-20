import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { auditRecent, auditVerify, type AuditEvent, type ChainVerification } from "../../shared/api/audit";
import { queryKeys } from "../../shared/api/queries";
import { EmptyState, MetricCard, errorToMessage, formatNumber } from "../../shared/ui/SharedComponents";

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
  const days = Math.round(hours / 24);
  return `${days}d ago`;
}

function shortHash(hash: string): string {
  if (!hash) return "-";
  return hash.length <= 16 ? hash : `${hash.slice(0, 10)}...${hash.slice(-6)}`;
}

function truncateDetail(value: string): string {
  const trimmed = value.trim();
  if (trimmed.length <= 220) return trimmed || "-";
  return `${trimmed.slice(0, 220)}...`;
}

function actionTone(actionType: string): PillTone {
  const lower = actionType.toLowerCase();
  if (/(block|deny|secret|tamper|broken|fail|error)/.test(lower)) return "danger";
  if (/(warn|network|paid|external|override)/.test(lower)) return "warn";
  if (/(verify|safe|approved|ok|passed)/.test(lower)) return "ok";
  if (/(agent|command|workflow|task|file|orchestrat)/.test(lower)) return "accent";
  return "neutral";
}

function integrityTone(verification: ChainVerification | undefined): "ok" | "warn" | "neutral" {
  if (!verification) return "neutral";
  return verification.valid ? "ok" : "warn";
}

function integrityValue(verification: ChainVerification | undefined, isLoading: boolean): string {
  if (isLoading) return "Checking";
  if (!verification) return "Unavailable";
  return verification.valid ? "Verified" : "Broken";
}

function AuditEventRow({ event }: { event: AuditEvent }) {
  return (
    <div className="table-row flex-col items-start gap-sm">
      <div className="w-full flex justify-between items-center gap-sm">
        <div className="flex-col gap-xs">
          <strong>{event.project_name || "Unknown project"}</strong>
          <span>{truncateDetail(event.details)}</span>
        </div>
        <span className={`pill ${actionTone(event.action_type)}`}>{event.action_type}</span>
      </div>
      <div className="row-meta">
        <span>{formatTimestamp(event.timestamp)}</span>
        <code title={event.hash}>{shortHash(event.hash)}</code>
      </div>
    </div>
  );
}

export function AuditTab() {
  const eventsQuery = useQuery({
    queryKey: queryKeys.audit.recent(RECENT_LIMIT),
    queryFn: () => auditRecent(RECENT_LIMIT),
  });
  const verificationQuery = useQuery({
    queryKey: queryKeys.audit.verify,
    queryFn: auditVerify,
  });

  const events = eventsQuery.data ?? [];
  const verification = verificationQuery.data;
  const isLoading = eventsQuery.isLoading || verificationQuery.isLoading;
  const isRefreshing = eventsQuery.isFetching || verificationQuery.isFetching;
  const error = eventsQuery.error ?? verificationQuery.error;
  const latest = events[0];

  const projectCount = useMemo(() => {
    const projects = new Set(events.map((event) => event.project_name).filter(Boolean));
    return projects.size;
  }, [events]);

  const refresh = () => {
    void eventsQuery.refetch();
    void verificationQuery.refetch();
  };

  return (
    <div className="content-grid dashboard-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Audit trail</p>
        <h1>{integrityValue(verification, isLoading)} hash chain.</h1>
        <p className="lead">
          RepoDesk reads the local JSONL audit trail and verifies every event hash against the
          previous event. No synthetic rows are shown here.
        </p>
        <div className="button-row">
          <button className="primary-button" onClick={refresh} disabled={isRefreshing}>
            {isRefreshing ? "Refreshing..." : "Refresh audit"}
          </button>
        </div>
      </section>

      {error && <div className="notice danger wide-panel">{errorToMessage(error)}</div>}
      {verification && !verification.valid && <div className="notice danger wide-panel">{verification.message}</div>}

      <div className="card-row">
        <MetricCard
          label="Chain integrity"
          value={integrityValue(verification, isLoading)}
          detail={verification?.message ?? "Waiting for verification result"}
          tone={integrityTone(verification)}
        />
        <MetricCard
          label="Total events"
          value={formatNumber(verification?.total_events ?? events.length)}
          detail={
            projectCount
              ? `Recent window spans ${projectCount} project${projectCount === 1 ? "" : "s"}`
              : "No project rows loaded"
          }
        />
        <MetricCard
          label="Last action"
          value={formatRelative(latest?.timestamp)}
          detail={latest ? `${latest.action_type} in ${latest.project_name || "Unknown"}` : "Nothing recorded yet"}
        />
      </div>

      <section className="panel wide-panel">
        <div className="panel-title-row compact">
          <div>
            <p className="eyebrow">Recent events</p>
            <h2>Newest first</h2>
          </div>
          <span className="pill neutral">{formatNumber(events.length)}</span>
        </div>
        {isLoading ? (
          <p className="muted">Loading audit trail...</p>
        ) : events.length === 0 ? (
          <EmptyState
            message="No audit events recorded yet."
            hint="Actions that write to the hash-chained audit log will appear here."
          />
        ) : (
          <div className="table-list">
            {events.map((event, index) => (
              <AuditEventRow key={`${event.hash}-${event.timestamp}-${index}`} event={event} />
            ))}
          </div>
        )}
      </section>

      {verification && (
        <section className="panel wide-panel">
          <div className="panel-title-row compact">
            <div>
              <p className="eyebrow">Verification receipt</p>
              <h2>SHA-256 chain status</h2>
            </div>
            <span className={`pill ${verification.valid ? "ok" : "danger"}`}>
              {verification.valid ? "valid" : "invalid"}
            </span>
          </div>
          <div className="route-summary-grid">
            <div>
              <span>Total events</span>
              <strong>{formatNumber(verification.total_events)}</strong>
            </div>
            <div>
              <span>Broken at</span>
              <strong>{verification.broken_at ?? "-"}</strong>
            </div>
          </div>
          <p className="muted m-0">{verification.message}</p>
        </section>
      )}
    </div>
  );
}
