import { ErrorState } from "../../shared/ui/ErrorState";
import type { MemoryEntry, MemoryProposal } from "../../shared/api/memory";
import { KIND_TONE } from "./constants";

export function ReviewQueue({
  proposals,
  entriesById,
  pendingCount,
  busy,
  reconcileError,
  onAccept,
  onReject,
  onReconcile,
}: {
  proposals: MemoryProposal[];
  entriesById: Map<number, MemoryEntry>;
  pendingCount: number;
  busy: boolean;
  reconcileError: unknown;
  onAccept: (id: number, keepId?: number | null) => void;
  onReject: (id: number) => void;
  onReconcile: (id: number) => void;
}) {
  return (
    <section className="panel wide-panel">
      <div className="panel-title-row">
        <div>
          <p className="eyebrow">Review queue</p>
          <h2>Pending proposals ({pendingCount})</h2>
        </div>
      </div>
      {pendingCount === 0 ? (
        <p className="muted">Nothing to review. Capture a response or run a scan.</p>
      ) : (
        <div className="table-list">
          {proposals.map((proposal) => (
            <ProposalRow
              key={proposal.id}
              proposal={proposal}
              entriesById={entriesById}
              onAccept={(keepId) => onAccept(proposal.id, keepId)}
              onReject={() => onReject(proposal.id)}
              onReconcile={() => onReconcile(proposal.id)}
              busy={busy}
            />
          ))}
        </div>
      )}
      {Boolean(reconcileError) && (
        <div style={{ marginTop: 8 }}>
          <ErrorState compact scope="memory:reconcile" error={reconcileError} />
        </div>
      )}
    </section>
  );
}

function ProposalRow({
  proposal,
  entriesById,
  onAccept,
  onReject,
  onReconcile,
  busy,
}: {
  proposal: MemoryProposal;
  entriesById: Map<number, MemoryEntry>;
  onAccept: (keepId?: number | null) => void;
  onReject: () => void;
  onReconcile: () => void;
  busy: boolean;
}) {
  const tone = KIND_TONE[proposal.kind] ?? "neutral";
  const sources = proposal.payload.source_ids
    .map((id) => entriesById.get(id))
    .filter((entry): entry is MemoryEntry => Boolean(entry));

  return (
    <div className="table-row flex-col items-start gap-sm" style={{ paddingBottom: 14 }}>
      <div className="w-full flex justify-between items-center">
        <span className={`pill ${tone}`}>{proposal.kind}</span>
        {proposal.payload.agent && <span className="muted" style={{ fontSize: 12 }}>{proposal.payload.agent}</span>}
      </div>
      <p style={{ margin: "4px 0" }}>{proposal.payload.rationale}</p>

      {proposal.payload.proposed && (
        <div className="route-list ok" style={{ width: "100%" }}>
          <strong>Proposed [{proposal.payload.proposed.category}]</strong>
          <span>{proposal.payload.proposed.content}</span>
        </div>
      )}

      {sources.length > 0 && (
        <div style={{ width: "100%", display: "flex", flexDirection: "column", gap: 4 }}>
          {sources.map((source) => (
            <div key={source.id} className="flex justify-between items-center" style={{ gap: 8 }}>
              <code style={{ flex: 1 }}>
                #{source.id} [{source.category}] {source.content}
              </code>
              {proposal.kind === "conflict" && (
                <button className="tiny-button" disabled={busy} onClick={() => onAccept(source.id)}>
                  Keep #{source.id}
                </button>
              )}
            </div>
          ))}
        </div>
      )}

      <ProposalActions proposal={proposal} busy={busy} onAccept={onAccept} onReject={onReject} onReconcile={onReconcile} />
    </div>
  );
}

function ProposalActions({
  proposal,
  busy,
  onAccept,
  onReject,
  onReconcile,
}: {
  proposal: MemoryProposal;
  busy: boolean;
  onAccept: (keepId?: number | null) => void;
  onReject: () => void;
  onReconcile: () => void;
}) {
  return (
    <div className="button-row compact-buttons" style={{ marginTop: 6 }}>
      {proposal.kind === "conflict" ? (
        <>
          <button
            className="tiny-button"
            disabled={busy}
            onClick={onReconcile}
            title="Use Ollama to write a single reconciled note"
          >
            Reconcile (Ollama)
          </button>
          {proposal.payload.proposed && (
            <button className="primary-button" disabled={busy} onClick={() => onAccept(null)}>
              Accept merged
            </button>
          )}
        </>
      ) : (
        <button className="primary-button" disabled={busy} onClick={() => onAccept(null)}>
          Accept
        </button>
      )}
      <button className="tiny-button" disabled={busy} onClick={onReject}>
        Reject
      </button>
    </div>
  );
}
