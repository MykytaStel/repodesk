import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as orchestrate from "../../shared/api/orchestrate";
import * as memory from "../../shared/api/memory";
import { queryKeys } from "../../shared/api/queries";

// Review surface: makes the run's output legible before it's committed — the
// exact files/diffs that changed, plus the memory the run proposed capturing
// (accept = add to memory) and a free-text note to add to memory by hand.
export function ReviewPanel({ runId, projectName }: { runId: string | null; projectName: string }) {
  const queryClient = useQueryClient();
  const [note, setNote] = useState("");

  const diffs = useQuery({
    queryKey: ["work", "review-diffs", runId],
    queryFn: () => (runId ? orchestrate.orchestrateRunDiffs(runId) : Promise.resolve([])),
    enabled: !!runId,
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

  const changedDiffs = (diffs.data ?? []).filter((d) => d.changed_files.length > 0);
  const pending = proposals.data ?? [];

  return (
    <div className="review-panel">
      {/* What changed — the exact files/diffs heading to a commit. */}
      <div className="review-block">
        <h4>What changed</h4>
        {!runId ? (
          <p className="muted">No run to review yet.</p>
        ) : diffs.isLoading ? (
          <p className="muted">Loading diff…</p>
        ) : changedDiffs.length === 0 ? (
          <p className="muted">No tracked file changes captured for this run.</p>
        ) : (
          changedDiffs.map((d) => (
            <details key={d.task_id} className="review-file">
              <summary>
                {d.task_id} — {d.changed_files.length} file(s): {d.changed_files.join(", ")}
              </summary>
              {d.diff.trim() ? (
                <pre className="review-diff">{d.diff}</pre>
              ) : (
                <p className="muted">No unified diff (new, binary, or already moved).</p>
              )}
              {d.truncated && <p className="muted">Diff truncated.</p>}
            </details>
          ))
        )}
      </div>

      {/* Memory — accept what the run proposed, or add a note by hand. */}
      <div className="review-block">
        <h4>Add to memory</h4>
        {proposals.isLoading ? (
          <p className="muted">Loading proposals…</p>
        ) : pending.length === 0 ? (
          <p className="muted">No pending memory proposals.</p>
        ) : (
          <ul className="proposal-list">
            {pending.map((p) => (
              <li key={p.id} className="proposal-item">
                <div>
                  <span className="proposal-kind">{p.kind}</span>
                  <p className="proposal-text">{p.payload.proposed?.content ?? p.payload.rationale}</p>
                </div>
                <div className="phase-actions">
                  <button
                    className="secondary-cta"
                    onClick={() => accept.mutate(p.id)}
                    disabled={accept.isPending}
                  >
                    Accept
                  </button>
                  <button
                    className="link-cta"
                    onClick={() => reject.mutate(p.id)}
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
            onChange={(e) => setNote(e.target.value)}
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
