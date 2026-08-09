import { lazy, Suspense, useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { orchestrationRuns, type RunSummary } from "../../shared/api/orchestrate";
import {
  linkAcceptanceEvidence,
  runEvidenceSnapshot,
  type AcceptanceCriterionEvidence,
  type RunEvidenceSnapshot,
  type VerificationCommandEvidence,
} from "../../shared/api/engineering";
import "./runs.css";

const OutcomesTab = lazy(() => import("../outcomes/OutcomesTab").then((m) => ({ default: m.OutcomesTab })));
const AuditTab = lazy(() => import("../audit/AuditTab").then((m) => ({ default: m.AuditTab })));

type RunsView = "runs" | "outcomes" | "audit";

const VIEWS: { id: RunsView; label: string }[] = [
  { id: "runs", label: "Run evidence" },
  { id: "outcomes", label: "Provider outcomes" },
  { id: "audit", label: "Raw audit" },
];

function fmtTime(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

function statusTone(value: string): "ok" | "danger" | "neutral" {
  if (["completed", "accepted", "passed", "proven"].includes(value)) return "ok";
  if (["failed", "rejected"].includes(value)) return "danger";
  return "neutral";
}

function shortId(value: string): string {
  return value.length > 24 ? `${value.slice(0, 21)}…` : value;
}

function RunListItem({ run, selected, onSelect }: {
  run: RunSummary;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <button className={`run-list-item${selected ? " selected" : ""}`} onClick={onSelect}>
      <div className="runs-list-header">
        <span className="run-id">{shortId(run.run_id)}</span>
        <span className={`pill ${statusTone(run.status)}`}>{run.status}</span>
      </div>
      <strong className="run-goal">{run.goal || "Untitled run"}</strong>
      <div className="run-list-meta">
        <span>{run.step_count} worker step{run.step_count === 1 ? "" : "s"}</span>
        <span>{fmtTime(run.started_at)}</span>
      </div>
    </button>
  );
}

function EvidencePaths({ paths }: { paths: string[] }) {
  if (paths.length === 0) return <span className="muted">None recorded</span>;
  return (
    <div className="evidence-paths">
      {paths.map((path) => <span className="evidence-path" key={path}>{path}</span>)}
    </div>
  );
}

function AcceptanceRow({
  criterion,
  commands,
  busy,
  onLink,
}: {
  criterion: AcceptanceCriterionEvidence;
  commands: VerificationCommandEvidence[];
  busy: boolean;
  onLink: (criterionId: string, command: string) => void;
}) {
  const [command, setCommand] = useState(commands[0]?.command ?? "");
  useEffect(() => {
    if (!commands.some((item) => item.command === command)) setCommand(commands[0]?.command ?? "");
  }, [commands, command]);

  return (
    <div className="evidence-row acceptance-row">
      <div className="acceptance-main">
        <div className="evidence-section-title">
          <strong>{criterion.criterion}</strong>
          <span className={`pill ${statusTone(criterion.status)}`}>{criterion.status.toUpperCase()}</span>
        </div>
        {criterion.command && <span className="evidence-mono">{criterion.command}</span>}
        {criterion.stale && (
          <span className="danger">Stale evidence: {criterion.stale_reason ?? "verification changed"}</span>
        )}
        {!criterion.command && criterion.status === "unproven" && (
          <span className="muted">No verification command is linked to this criterion.</span>
        )}
      </div>
      {criterion.status !== "proven" && commands.length > 0 && (
        <div className="acceptance-action">
          <select value={command} onChange={(event) => setCommand(event.target.value)} disabled={busy}>
            {commands.map((item) => (
              <option key={item.command} value={item.command}>
                {item.success ? "✓" : "✕"} {item.command}
              </option>
            ))}
          </select>
          <button
            className="tiny-button"
            disabled={busy || !command}
            onClick={() => onLink(criterion.criterion_id, command)}
          >
            Link proof
          </button>
        </div>
      )}
    </div>
  );
}

function RunEvidenceDetail({
  evidence,
  linking,
  onLink,
}: {
  evidence: RunEvidenceSnapshot;
  linking: boolean;
  onLink: (criterionId: string, command: string) => void;
}) {
  const totalTokens = evidence.total_input_tokens + evidence.total_output_tokens;
  const verificationIsCurrent = ["passed", "failed"].includes(evidence.verification.state);
  const linkableCommands = verificationIsCurrent ? evidence.verification.commands : [];

  return (
    <div className="run-evidence-detail">
      <div className="run-evidence-header">
        <div>
          <p className="eyebrow">Run evidence</p>
          <h2>{evidence.goal || evidence.run_id}</h2>
          <p className="muted evidence-mono">{evidence.run_id}</p>
        </div>
        <span className={`pill ${statusTone(evidence.status)}`}>{evidence.status}</span>
      </div>

      <div className="run-evidence-facts">
        <span className="run-evidence-fact"><strong>{evidence.workers.length}</strong> workers</span>
        <span className="run-evidence-fact"><strong>{evidence.changed_files.length}</strong> changed files</span>
        <span className="run-evidence-fact"><strong>{totalTokens.toLocaleString()}</strong> tokens</span>
        <span className="run-evidence-fact"><strong>{evidence.total_cost_units.toFixed(4)}</strong> cost units</span>
        <span>{fmtTime(evidence.started_at)}</span>
      </div>

      <section className="evidence-section">
        <div className="evidence-section-title">
          <h3>Context</h3>
          <span className="evidence-source">{evidence.context.source}</span>
        </div>
        <div className="evidence-row-meta">
          <span>{evidence.context.estimated_tokens?.toLocaleString() ?? "Unknown"} estimated tokens</span>
          <span>{evidence.context.evidence.length} evidence refs</span>
        </div>
      </section>

      <section className="evidence-section">
        <div className="evidence-section-title"><h3>Workers</h3><span>{evidence.workers.length}</span></div>
        <div className="evidence-list">
          {evidence.workers.map((worker) => (
            <div className="evidence-row" key={worker.step_id}>
              <div className="evidence-section-title">
                <strong>{worker.agent || worker.provider || worker.step_id}</strong>
                <span className={`pill ${statusTone(worker.status)}`}>{worker.status}</span>
              </div>
              <div className="evidence-row-meta">
                <span>{worker.provider}{worker.model ? ` / ${worker.model}` : ""}</span>
                <span>{(worker.input_tokens + worker.output_tokens).toLocaleString()} tokens</span>
                <span>{worker.changed_files.length} files</span>
              </div>
            </div>
          ))}
        </div>
      </section>

      <section className="evidence-section">
        <div className="evidence-section-title"><h3>Changes</h3><span>{evidence.changed_files.length}</span></div>
        <EvidencePaths paths={evidence.changed_files} />
      </section>

      <section className="evidence-section">
        <div className="evidence-section-title">
          <h3>Human review</h3>
          <span className={`pill ${statusTone(evidence.review.state)}`}>{evidence.review.state}</span>
        </div>
        <div className="evidence-row-meta">
          <span className="evidence-source">source: {evidence.review.source}</span>
          <span>{evidence.review.reviewed_paths.length} reviewed files</span>
        </div>
      </section>

      <section className="evidence-section">
        <div className="evidence-section-title">
          <h3>Verification</h3>
          <span className={`pill ${statusTone(evidence.verification.state)}`}>{evidence.verification.state}</span>
        </div>
        <div className="evidence-row-meta">
          <span className="evidence-source">source: {evidence.verification.source}</span>
          {evidence.verification.verified_at && <span>{fmtTime(evidence.verification.verified_at)}</span>}
        </div>
        <div className="evidence-list">
          {evidence.verification.commands.map((check) => (
            <div className="evidence-row" key={check.command}>
              <div className="evidence-section-title">
                <code>{check.command}</code>
                <span className={`pill ${check.success ? "ok" : "danger"}`}>{check.success ? "passed" : "failed"}</span>
              </div>
            </div>
          ))}
          {evidence.verification.commands.length === 0 && (
            <div className="evidence-empty">No command-level receipt is available for this run.</div>
          )}
        </div>
      </section>

      <section className="evidence-section">
        <div className="evidence-section-title">
          <h3>Acceptance evidence</h3>
          <span>
            <span className="ok">{evidence.acceptance.proven} proven</span>
            {" · "}<span className="danger">{evidence.acceptance.failed} failed</span>
            {" · "}<span>{evidence.acceptance.unproven} unproven</span>
          </span>
        </div>
        {evidence.verification.state === "stale" && (
          <div className="evidence-empty">Re-run verification before linking new acceptance proof.</div>
        )}
        {!evidence.acceptance.configured ? (
          <div className="evidence-empty">Configure acceptance criteria in the Work Item Engineering Contract.</div>
        ) : evidence.acceptance.criteria.length === 0 ? (
          <div className="evidence-empty">The Engineering Contract has no acceptance criteria.</div>
        ) : (
          <div className="evidence-list">
            {evidence.acceptance.criteria.map((criterion) => (
              <AcceptanceRow
                key={criterion.criterion_id}
                criterion={criterion}
                commands={linkableCommands}
                busy={linking}
                onLink={onLink}
              />
            ))}
          </div>
        )}
      </section>

      <section className="evidence-section">
        <div className="evidence-section-title">
          <h3>Commit</h3>
          <span className={`pill ${evidence.commit.committed ? "ok" : "neutral"}`}>
            {evidence.commit.committed ? "committed" : "not committed"}
          </span>
        </div>
        <div className="evidence-row-meta">
          <span className="evidence-source">source: {evidence.commit.source}</span>
          {evidence.commit.commit_sha && <span className="evidence-mono">{evidence.commit.commit_sha}</span>}
        </div>
      </section>
    </div>
  );
}

function RunsWorkspace() {
  const queryClient = useQueryClient();
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null);
  const runs = useQuery({
    queryKey: ["runs", "list"],
    queryFn: orchestrationRuns,
    staleTime: 5_000,
    refetchOnWindowFocus: true,
  });

  useEffect(() => {
    const list = runs.data ?? [];
    if (list.length === 0) {
      if (selectedRunId) setSelectedRunId(null);
      return;
    }
    if (!selectedRunId || !list.some((run) => run.run_id === selectedRunId)) {
      setSelectedRunId(list[0].run_id);
    }
  }, [runs.data, selectedRunId]);

  const detail = useQuery({
    queryKey: ["runs", "evidence", selectedRunId],
    queryFn: () => runEvidenceSnapshot(selectedRunId!),
    enabled: Boolean(selectedRunId),
    staleTime: 2_000,
    refetchOnWindowFocus: true,
  });

  const link = useMutation({
    mutationFn: ({ criterionId, command }: { criterionId: string; command: string }) =>
      linkAcceptanceEvidence(selectedRunId!, criterionId, command),
    onSuccess: (next) => {
      queryClient.setQueryData(["runs", "evidence", selectedRunId], next);
    },
  });

  if (runs.isLoading) return <p className="muted">Loading persisted runs…</p>;
  if (runs.isError) {
    return <div className="evidence-empty">No active Work Item, or its run history could not be loaded.</div>;
  }

  const list = runs.data ?? [];
  if (list.length === 0) {
    return <div className="evidence-empty">No persisted execution runs yet. Execute the active Work Item to create evidence.</div>;
  }

  const refresh = () => {
    void runs.refetch();
    if (selectedRunId) void detail.refetch();
  };

  return (
    <div className="runs-shell">
      <aside className="runs-list">
        <div className="runs-list-header">
          <strong>Persisted runs</strong>
          <div className="button-row" style={{ marginTop: 0 }}>
            <span className="pill">{list.length}</span>
            <button type="button" className="tiny-button" onClick={refresh}>Refresh</button>
          </div>
        </div>
        {list.map((run) => (
          <RunListItem
            key={run.run_id}
            run={run}
            selected={selectedRunId === run.run_id}
            onSelect={() => setSelectedRunId(run.run_id)}
          />
        ))}
      </aside>

      {detail.isLoading ? (
        <div className="run-evidence-detail muted">Loading run evidence…</div>
      ) : detail.isError || !detail.data ? (
        <div className="run-evidence-detail danger">Run evidence could not be reconstructed.</div>
      ) : (
        <RunEvidenceDetail
          evidence={detail.data}
          linking={link.isPending}
          onLink={(criterionId, command) => link.mutate({ criterionId, command })}
        />
      )}
    </div>
  );
}

export function HistoryTab() {
  const [view, setView] = useState<RunsView>("runs");
  return (
    <div className="subnav-host history-tab">
      <div className="changes-summary">
        <div>
          <p className="eyebrow">Runs</p>
          <strong>Execution history with inspectable engineering evidence</strong>
        </div>
      </div>
      <div className="subnav" role="tablist" aria-label="Runs views">
        {VIEWS.map((item) => (
          <button
            key={item.id}
            role="tab"
            aria-selected={view === item.id}
            className={view === item.id ? "selected" : ""}
            onClick={() => setView(item.id)}
          >
            {item.label}
          </button>
        ))}
      </div>
      <Suspense fallback={<p className="muted">Loading…</p>}>
        {view === "runs" ? <RunsWorkspace /> : view === "outcomes" ? <OutcomesTab /> : <AuditTab />}
      </Suspense>
    </div>
  );
}
