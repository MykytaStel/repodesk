import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { orchestrationRuns, type RunSummary } from "../../shared/api/orchestrate";
import type {
  AcceptanceCriterionEvidence,
  RunEvidenceSnapshot,
  VerificationCommandEvidence,
} from "../../shared/api/engineering";
import {
  linkAcceptanceEvidenceBundle,
  runEvidenceBundle,
  type RunObservabilityReport,
} from "../../shared/api/observability";
import {
  EmptyState,
  ErrorState,
  EvidenceState,
  LoadingState,
  Metric,
  PanelHeader,
  StatusBadge,
} from "../../shared/ui/primitives";
import {
  acceptanceCriterionSemantic,
  commitSemantic,
  normalizeRunReviewState,
  normalizeRunVerificationState,
  reviewStateSemantic,
  runDispositionSemantic,
  runStatusSemantic,
  verificationCommandSemantic,
  verificationStateSemantic,
  workerStatusSemantic,
} from "./runsSemantic";

function fmtTime(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

function shortId(value: string): string {
  return value.length > 24 ? `${value.slice(0, 21)}…` : value;
}

function pct(value: number | null): string {
  return value == null ? "—" : `${Math.round(value * 100)}%`;
}

function compactNumber(value: number | null, digits = 1): string {
  return value == null ? "—" : value.toLocaleString(undefined, { maximumFractionDigits: digits });
}

function RunListItem({ run, selected, onSelect }: {
  run: RunSummary;
  selected: boolean;
  onSelect: () => void;
}) {
  const semantic = runStatusSemantic(run.status);
  return (
    <button className={`run-list-item${selected ? " selected" : ""}`} onClick={onSelect}>
      <div className="runs-list-header">
        <span className="run-id">{shortId(run.run_id)}</span>
        <StatusBadge label={semantic.label} tone={semantic.tone} />
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
  const semantic = acceptanceCriterionSemantic(criterion.status, criterion.stale);

  useEffect(() => {
    if (!commands.some((item) => item.command === command)) setCommand(commands[0]?.command ?? "");
  }, [commands, command]);

  return (
    <div className="evidence-row acceptance-row">
      <div className="acceptance-main">
        <div className="evidence-section-title">
          <strong>{criterion.criterion}</strong>
          <StatusBadge label={semantic.label} tone={semantic.tone} />
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

function RunObservability({ report }: { report: RunObservabilityReport }) {
  const { disposition, efficiency, context, strategy } = report;
  const dispositionSemantic = runDispositionSemantic(disposition.state);

  return (
    <>
      <section className={`run-disposition ${disposition.state}`}>
        <EvidenceState
          label={`Current disposition · ${disposition.stage}`}
          state={dispositionSemantic.label}
          tone={dispositionSemantic.tone}
        >
          <h3>{disposition.title}</h3>
          <p>{disposition.detail}</p>
        </EvidenceState>
      </section>

      {strategy ? (
        <section className="run-strategy-evidence" aria-label="AI strategy used by this run">
          <div>
            <span>Strategy</span>
            <strong>{strategy.requested_mode} → {strategy.resolved_profile}</strong>
            <code title={strategy.plan_fingerprint}>{shortId(strategy.plan_fingerprint)}</code>
          </div>
          <div>
            <span>AI calls</span>
            <strong>{strategy.baseline_steps} → {strategy.planned_steps}</strong>
            <code>{strategy.plan_shape}</code>
          </div>
          <div>
            <span>Predicted saving</span>
            <strong>{strategy.estimated_saved_tokens.toLocaleString()} tok</strong>
            <code>before execution</code>
          </div>
          <div>
            <span>Context lock</span>
            <strong>{strategy.context_fingerprint ? shortId(strategy.context_fingerprint) : "legacy"}</strong>
            <code>{strategy.context_fingerprint ? "bound" : "not recorded"}</code>
          </div>
        </section>
      ) : null}

      <section className="run-observability-grid" aria-label="Run efficiency">
        <Metric label="Workers" value={String(efficiency.workers)} detail={`${efficiency.successful_workers} successful`} />
        <Metric label="Failed / blocked" value={String(efficiency.failed_workers + efficiency.blocked_workers)} detail={`${efficiency.skipped_workers} skipped`} tone={(efficiency.failed_workers + efficiency.blocked_workers) > 0 ? "critical" : "neutral"} />
        <Metric label="Handoffs" value={String(efficiency.handoffs)} detail={`${efficiency.unique_providers} provider(s)`} />
        <Metric label="Tokens / changed file" value={compactNumber(efficiency.tokens_per_changed_file, 0)} detail={`${efficiency.total_tokens.toLocaleString()} total`} />
        <Metric label="Input / output" value={efficiency.input_output_ratio == null ? "—" : `${compactNumber(efficiency.input_output_ratio)}:1`} detail={`${efficiency.unique_models} model(s)`} />
        <Metric label="Repeated context" value={pct(context.repeated_context_ratio)} detail={`${context.repeated_tokens?.toLocaleString() ?? "—"} tokens`} tone={(context.repeated_context_ratio ?? 0) > 0.25 ? "attention" : "neutral"} />
      </section>
    </>
  );
}

function RunEvidenceDetail({
  evidence,
  observability,
  linking,
  linkError,
  onLink,
}: {
  evidence: RunEvidenceSnapshot;
  observability: RunObservabilityReport;
  linking: boolean;
  linkError: string | null;
  onLink: (criterionId: string, command: string) => void;
}) {
  const totalTokens = evidence.total_input_tokens + evidence.total_output_tokens;
  const runSemantic = runStatusSemantic(evidence.status);
  const reviewSemantic = reviewStateSemantic(normalizeRunReviewState(evidence.review.state));
  const verificationState = normalizeRunVerificationState(evidence.verification.state);
  const verificationSemantic = verificationStateSemantic(verificationState);
  const commitState = commitSemantic(evidence.commit.committed);
  const verificationIsCurrent = verificationState === "passed" || verificationState === "failed";
  const linkableCommands = verificationIsCurrent ? evidence.verification.commands : [];

  return (
    <div className="run-evidence-detail">
      <div className="run-evidence-header">
        <PanelHeader
          eyebrow="Run evidence"
          title={evidence.goal || evidence.run_id}
          description={<span className="evidence-mono">{evidence.run_id}</span>}
          trailing={<StatusBadge label={runSemantic.label} tone={runSemantic.tone} />}
        />
      </div>

      <div className="run-evidence-facts">
        <Metric label="Workers" value={String(evidence.workers.length)} />
        <Metric label="Changed files" value={String(evidence.changed_files.length)} />
        <Metric label="Tokens" value={totalTokens.toLocaleString()} />
        <Metric label="Cost units" value={evidence.total_cost_units.toFixed(4)} />
        <span>{fmtTime(evidence.started_at)}</span>
      </div>

      {linkError ? <ErrorState title="Acceptance evidence link failed" detail={linkError} /> : null}

      <RunObservability report={observability} />

      <section className="evidence-section">
        <div className="evidence-section-title">
          <h3>Context</h3>
          <span className="evidence-source">{evidence.context.source}</span>
        </div>
        <div className="evidence-row-meta run-context-metrics">
          <span>{observability.context.included_tokens?.toLocaleString() ?? evidence.context.estimated_tokens?.toLocaleString() ?? "Unknown"} included</span>
          <span>{observability.context.candidate_tokens?.toLocaleString() ?? "Unknown"} candidate</span>
          <span>{observability.context.compacted_tokens?.toLocaleString() ?? "Unknown"} compacted</span>
          <span>{pct(observability.context.repeated_context_ratio)} repeated</span>
          <span>{evidence.context.evidence.length} evidence refs</span>
        </div>
      </section>

      <section className="evidence-section">
        <div className="evidence-section-title"><h3>Workers</h3><span>{evidence.workers.length}</span></div>
        <div className="evidence-list">
          {evidence.workers.map((worker) => {
            const semantic = workerStatusSemantic(worker.status);
            return (
              <div className="evidence-row" key={worker.step_id}>
                <div className="evidence-section-title">
                  <strong>{worker.agent || worker.provider || worker.step_id}</strong>
                  <StatusBadge label={semantic.label} tone={semantic.tone} />
                </div>
                <div className="evidence-row-meta">
                  <span>{worker.provider}{worker.model ? ` / ${worker.model}` : ""}</span>
                  <span>{(worker.input_tokens + worker.output_tokens).toLocaleString()} tokens</span>
                  <span>{worker.changed_files.length} files</span>
                </div>
              </div>
            );
          })}
        </div>
      </section>

      <section className="evidence-section">
        <div className="evidence-section-title"><h3>Changes</h3><span>{evidence.changed_files.length}</span></div>
        <EvidencePaths paths={evidence.changed_files} />
      </section>

      <section className="evidence-section">
        <div className="evidence-section-title">
          <h3>Human review</h3>
          <StatusBadge label={reviewSemantic.label} tone={reviewSemantic.tone} />
        </div>
        <div className="evidence-row-meta">
          <span className="evidence-source">source: {evidence.review.source}</span>
          <span>{evidence.review.reviewed_paths.length} reviewed files</span>
        </div>
      </section>

      <section className="evidence-section">
        <div className="evidence-section-title">
          <h3>Verification</h3>
          <StatusBadge label={verificationSemantic.label} tone={verificationSemantic.tone} />
        </div>
        <div className="evidence-row-meta">
          <span className="evidence-source">source: {evidence.verification.source}</span>
          {evidence.verification.verified_at && <span>{fmtTime(evidence.verification.verified_at)}</span>}
        </div>
        <div className="evidence-list">
          {evidence.verification.commands.map((check) => {
            const semantic = verificationCommandSemantic(check.success);
            return (
              <div className="evidence-row" key={check.command}>
                <div className="evidence-section-title">
                  <code>{check.command}</code>
                  <StatusBadge label={semantic.label} tone={semantic.tone} />
                </div>
              </div>
            );
          })}
          {evidence.verification.commands.length === 0 && (
            <EmptyState message="No command-level receipt is available for this run." />
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
        {verificationState === "stale" && (
          <EmptyState message="Re-run verification before linking new acceptance proof." />
        )}
        {!evidence.acceptance.configured ? (
          <EmptyState message="Configure acceptance criteria in the Work Item Engineering Contract." />
        ) : evidence.acceptance.criteria.length === 0 ? (
          <EmptyState message="The Engineering Contract has no acceptance criteria." />
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
          <StatusBadge label={commitState.label} tone={commitState.tone} />
        </div>
        <div className="evidence-row-meta">
          <span className="evidence-source">source: {evidence.commit.source}</span>
          {evidence.commit.commit_sha && <span className="evidence-mono">{evidence.commit.commit_sha}</span>}
        </div>
      </section>
    </div>
  );
}

export function RunsWorkspace() {
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
    queryKey: ["runs", "observability", selectedRunId],
    queryFn: () => runEvidenceBundle(selectedRunId!),
    enabled: Boolean(selectedRunId),
    staleTime: 2_000,
    refetchOnWindowFocus: true,
  });

  const link = useMutation({
    mutationFn: ({ criterionId, command }: { criterionId: string; command: string }) =>
      linkAcceptanceEvidenceBundle(selectedRunId!, criterionId, command),
    onSuccess: (next) => {
      queryClient.setQueryData(["runs", "observability", selectedRunId], next);
    },
  });

  if (runs.isLoading) return <LoadingState scope="surface" message="Loading persisted runs…" />;
  if (runs.isError) {
    const detail = runs.error instanceof Error ? runs.error.message : String(runs.error);
    return <ErrorState scope="surface" title="Run history unavailable" detail={detail} />;
  }

  const list = runs.data ?? [];
  if (list.length === 0) {
    return (
      <EmptyState
        scope="surface"
        message="No persisted execution runs yet."
        hint="Execute the active Work Item to create immutable run evidence."
      />
    );
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
          <div className="button-row">
            <StatusBadge label={String(list.length)} tone="neutral" ariaLabel={`${list.length} persisted runs`} />
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
        <div className="run-evidence-detail"><LoadingState message="Loading run evidence…" /></div>
      ) : detail.isError || !detail.data ? (
        <div className="run-evidence-detail">
          <ErrorState
            title="Run evidence unavailable"
            detail={detail.error instanceof Error ? detail.error.message : "Run evidence or observability could not be reconstructed."}
          />
        </div>
      ) : (
        <RunEvidenceDetail
          evidence={detail.data.evidence}
          observability={detail.data.observability}
          linking={link.isPending}
          linkError={link.error instanceof Error ? link.error.message : link.error ? String(link.error) : null}
          onLink={(criterionId, command) => link.mutate({ criterionId, command })}
        />
      )}
    </div>
  );
}
