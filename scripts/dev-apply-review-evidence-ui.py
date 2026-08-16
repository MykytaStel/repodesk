from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}: {old[:120]!r}")
    file.write_text(text.replace(old, new, 1))


# Tauri: expose the core's read-only evidence state through one bounded command.
commands = "apps/desktop/src-tauri/src/commands/orchestrate.rs"
replace_once(
    commands,
    '''#[tauri::command]
pub fn orchestrate_run_diffs(run_id: String) -> Result<Vec<RunDiff>, ErrorPayload> {
''',
    '''#[tauri::command]
pub fn orchestrate_evidence_state(
    run_id: String,
) -> Result<orchestrator::ExecutionEvidenceState, ErrorPayload> {
    validate_run_id(&run_id)?;
    Ok(orchestrator::evidence_state_for_run(&run_id)?)
}

#[tauri::command]
pub fn orchestrate_run_diffs(run_id: String) -> Result<Vec<RunDiff>, ErrorPayload> {
''',
)

lib = "apps/desktop/src-tauri/src/lib.rs"
replace_once(
    lib,
    '''            commands::orchestrate_show,
            commands::orchestrate_review,
            commands::orchestrate_run_diffs,
''',
    '''            commands::orchestrate_show,
            commands::orchestrate_review,
            commands::orchestrate_evidence_state,
            commands::orchestrate_run_diffs,
''',
)

# TypeScript contract: keep backend provenance/evidence vocabulary exact.
api = "apps/desktop/src/shared/api/orchestrate.ts"
replace_once(
    api,
    '''export type SubAgentStatus = "ok" | "skipped" | "blocked" | "failed";
export type RunStatus = "completed" | "partial" | "failed" | "dry_run";
''',
    '''export type SubAgentStatus = "ok" | "skipped" | "blocked" | "failed";
export type RunStatus = "completed" | "partial" | "failed" | "dry_run";
export type ChangeEvidenceStatus = "complete" | "unavailable" | "legacy_unknown";
export type ExecutionEvidenceStatus = "ready" | "recovery_required" | "incomplete" | "not_required";

export type ExecutionEvidenceState = {
  run_id: string;
  status: ExecutionEvidenceStatus;
  recoverable: boolean;
  detail?: string | null;
};
''',
)
replace_once(
    api,
    '''  captured_proposals: number;
  changed_files?: string[];
  diff_path?: string | null;
''',
    '''  captured_proposals: number;
  changed_files?: string[];
  change_evidence_status?: ChangeEvidenceStatus;
  execution_issues?: string[];
  diff_path?: string | null;
''',
)
replace_once(
    api,
    '''export type LoopStatus = "succeeded" | "needs_approval" | "guardrail_blocked" | "exhausted" | "dry_run";
''',
    '''export type LoopStatus =
  | "succeeded"
  | "needs_approval"
  | "guardrail_blocked"
  | "evidence_recovery_required"
  | "exhausted"
  | "dry_run";
''',
)
replace_once(
    api,
    '''export async function orchestrateRunDiffs(runId: string): Promise<RunDiff[]> {
  return invoke("orchestrate_run_diffs", { runId });
}
''',
    '''export async function orchestrateEvidenceState(runId: string): Promise<ExecutionEvidenceState> {
  return invoke("orchestrate_evidence_state", { runId });
}

export async function orchestrateRunDiffs(runId: string): Promise<RunDiff[]> {
  return invoke("orchestrate_run_diffs", { runId });
}
''',
)

# Review: evidence preflight is the source of truth. Diff IPC is intentionally
# lazy because reading persisted diffs is unnecessary and misleading until the
# receipt is review-safe.
review = Path("apps/desktop/src/features/work/ReviewPanel.tsx")
review.write_text('''import { useState } from "react";
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
''')
