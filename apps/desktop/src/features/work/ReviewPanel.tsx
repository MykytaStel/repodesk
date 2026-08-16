import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as orchestrate from "../../shared/api/orchestrate";
import * as memory from "../../shared/api/memory";
import { queryKeys } from "../../shared/api/queries";
import { DiffViewer } from "../../shared/ui/DiffViewer";

// Review surface: evidence is checked before any diff is read. This keeps the
// UI fail-closed and prevents an empty/unavailable changeset from being shown
// as proof that the agent made no writes.
export function ReviewPanel({ runId, projectName }: { runId: string | null; projectName: string }) {
  const queryClient = useQueryClient();
  const [note, setNote] = useState("");

  const evidence = useQuery({
    queryKey: ["work", "review-evidence", runId],
    queryFn: () => orchestrate.orchestrateEvidenceState(runId ?? ""),
    enabled: !!runId,
  });
  const evidenceReady = evidence.data?.status === "ready";

  const diffs = useQuery({
    queryKey: ["work", "review-diffs", runId],
    queryFn: () => orchestrate.orchestrateRunDiffs(runId ?? ""),
    enabled: !!runId && evidenceReady,
  });

  const proposals = useQuery({
    queryKey: queryKeys.memory.proposals(projectName),
    queryFn: () => (projectName ? memory.listMemoryProposals(projectName, false) : Promise.resolve([])),
    enabled: !!projectName,
  });

  const refreshProposals = () =>
    queryClient.invalidateQueries({ queryKey: queryKeys.memory.proposals(projectName) });

  const accept = useMutation({
    mutationFn: (id: number) => memory.acceptMemoryProposal(id),
    onSuccess: refreshProposals,
  });
  const reject = useMutation({
    mutationFn: (id: number) => memory.rejectMemoryProposal(id),
    onSuccess: refreshProposals,
  });
  const addNote = useMutation({
    mutationFn: (content: string) => memory.appendProjectMemory(projectName, content),
    onSuccess: () => {
      setNote("");
      refreshProposals();
    },
  });

  const changedDiffs = (diffs.data ?? []).filter((diff) => diff.changed_files.length > 0);
  const pending = proposals.data ?? [];

  const evidenceContent = !runId ? (
    <p className="muted">No run to review yet.</p>
  ) : evidence.isLoading ? (
    <p className="muted">Checking execution evidence…</p>
  ) : evidence.isError || !evidence.data ? (
    <p className="muted" role="alert">
      Evidence status unavailable. Review is blocked until execution evidence can be verified.
    </p>
  ) : evidence.data.status === "incomplete" ? (
    <p className="muted" role="alert">
      Change evidence unavailable. RepoDesk cannot prove which tracked paths changed. Rerun execution to capture a
      trustworthy changeset.
    </p>
  ) : evidence.data.status === "recovery_required" ? (
    <p className="muted" role="alert">
      Execution finished, but the persisted receipt needs repair. Repair execution evidence; do not rerun the agent.
    </p>
  ) : evidence.data.status === "not_required" ? (
    <p className="muted" role="alert">
      This was a dry run, so there is no reviewable execution evidence.
    </p>
  ) : diffs.isLoading ? (
    <p className="muted">Loading diff…</p>
  ) : diffs.isError ? (
    <p className="muted" role="alert">
      Diff evidence could not be loaded. Review is blocked until the recorded changes can be read.
    </p>
  ) : changedDiffs.length === 0 ? (
    <p className="muted">Changeset capture is complete; no tracked file changes were produced.</p>
  ) : (
    changedDiffs.map((diff) => (
      <details key={diff.task_id} className="review-file">
        <summary>
          {diff.task_id} — {diff.changed_files.length} file(s): {diff.changed_files.join(", ")}
        </summary>
        {diff.diff.trim() ? (
          <DiffViewer diff={diff.diff} />
        ) : (
          <p className="muted">No unified diff (new, binary, or already moved).</p>
        )}
        {diff.truncated && <p className="muted">Diff truncated.</p>}
      </details>
    ))
  );

  return (
    <div className="review-panel">
      <div className="review-block">
        <h4>What changed</h4>
        {evidenceContent}
      </div>

      {/* Memory proposals remain useful metadata even when file evidence is blocked. */}
      <div className="review-block">
        <h4>Add to memory</h4>
        {proposals.isLoading ? (
          <p className="muted">Loading proposals…</p>
        ) : pending.length === 0 ? (
          <p className="muted">No pending memory proposals.</p>
        ) : (
          <ul className="proposal-list">
            {pending.map((proposal) => (
              <li key={proposal.id} className="proposal-item">
                <div>
                  <span className="proposal-kind">{proposal.kind}</span>
                  <p className="proposal-text">
                    {proposal.payload.proposed?.content ?? proposal.payload.rationale}
                  </p>
                </div>
                <div className="phase-actions">
                  <button
                    className="secondary-cta"
                    onClick={() => accept.mutate(proposal.id)}
                    disabled={accept.isPending}
                  >
                    Accept
                  </button>
                  <button
                    className="link-cta"
                    onClick={() => reject.mutate(proposal.id)}
                    disabled={reject.isPending}
                  >
                    Reject
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
        <div className="phase-actions add-note-row">
          <input
            className="commit-input"
            placeholder="Add a note to memory…"
            value={note}
            onChange={(event) => setNote(event.target.value)}
          />
          <button
            className="secondary-cta"
            onClick={() => addNote.mutate(note.trim())}
            disabled={addNote.isPending || note.trim().length === 0}
          >
            Add note
          </button>
        </div>
      </div>
    </div>
  );
}
