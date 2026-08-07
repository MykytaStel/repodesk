import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ENGINEERING_KNOWLEDGE_KEY,
  acceptEngineeringKnowledge,
  archiveEngineeringKnowledge,
  captureVerifiedKnowledgeCommand,
  engineeringKnowledgeSnapshot,
  proposeEngineeringKnowledge,
  type EngineeringKnowledgeCategory,
  type EngineeringKnowledgeRecord,
  type EngineeringKnowledgeSnapshot,
  type EngineeringKnowledgeStatus,
} from "../../shared/api/knowledge";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { errorToMessage } from "../../shared/utils/helpers";
import "./knowledge.css";

const CATEGORY_LABELS: Record<EngineeringKnowledgeCategory, string> = {
  architecture: "Architecture",
  convention: "Convention",
  hazard: "Hazard",
  command: "Command",
  testing: "Testing",
  decision: "Decision",
  performance: "Performance",
  tooling: "Tooling",
};

const STATUS_LABELS: Record<EngineeringKnowledgeStatus, string> = {
  candidate: "Candidate",
  accepted: "Accepted",
  archived: "Archived",
};

type Filter = "review" | "accepted" | "archived" | "all";

function toneForStatus(status: EngineeringKnowledgeStatus): string {
  if (status === "accepted") return "ok";
  if (status === "candidate") return "warn";
  return "neutral";
}

function applySnapshot(
  queryClient: ReturnType<typeof useQueryClient>,
  snapshot: EngineeringKnowledgeSnapshot,
) {
  queryClient.setQueryData(ENGINEERING_KNOWLEDGE_KEY, snapshot);
  void queryClient.invalidateQueries({ queryKey: ["work"] });
}

function KnowledgeListItem({
  record,
  selected,
  onSelect,
}: {
  record: EngineeringKnowledgeRecord;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      className={`knowledge-list-item${selected ? " selected" : ""}`}
      onClick={onSelect}
    >
      <div className="knowledge-list-line">
        <span className={`knowledge-category category-${record.category}`}>
          {CATEGORY_LABELS[record.category]}
        </span>
        <span className={`pill ${toneForStatus(record.status)}`}>{STATUS_LABELS[record.status]}</span>
      </div>
      <strong>{record.title}</strong>
      <small>
        {record.origin === "verification" ? "From verification" : "Human proposed"}
        {record.source_work_item_id ? ` · ${record.source_work_item_id}` : ""}
      </small>
    </button>
  );
}

function KnowledgeDetail({
  record,
  busy,
  onAccept,
  onArchive,
}: {
  record: EngineeringKnowledgeRecord;
  busy: boolean;
  onAccept: () => void;
  onArchive: () => void;
}) {
  return (
    <article className="knowledge-detail">
      <header className="knowledge-detail-header">
        <div>
          <span className={`knowledge-category category-${record.category}`}>
            {CATEGORY_LABELS[record.category]}
          </span>
          <h2>{record.title}</h2>
          <p className="muted">
            {record.origin === "verification" ? "Captured from verified engineering evidence" : "Human-proposed project knowledge"}
          </p>
        </div>
        <span className={`pill ${toneForStatus(record.status)}`}>{STATUS_LABELS[record.status]}</span>
      </header>

      <section className="knowledge-detail-section">
        <span className="knowledge-section-label">Knowledge</span>
        <p className="knowledge-content">{record.content}</p>
      </section>

      <section className="knowledge-detail-section">
        <span className="knowledge-section-label">Provenance</span>
        <div className="knowledge-provenance-grid">
          <span>Project</span><strong>{record.project}</strong>
          <span>Source Work Item</span><strong>{record.source_work_item_id ?? "Not bound"}</strong>
          <span>Evidence refs</span><strong>{record.evidence.length}</strong>
          <span>Updated</span><strong>{new Date(record.updated_at).toLocaleString()}</strong>
        </div>
        {record.evidence.length > 0 ? (
          <div className="knowledge-evidence-list">
            {record.evidence.map((evidence) => (
              <div className="knowledge-evidence-row" key={`${evidence.kind}:${evidence.locator}`}>
                <span>{evidence.kind}</span>
                <code>{evidence.locator}</code>
              </div>
            ))}
          </div>
        ) : (
          <p className="muted">No machine evidence attached. This record was proposed manually.</p>
        )}
      </section>

      <section className="knowledge-detail-section knowledge-context-state">
        <span className="knowledge-section-label">Agent context</span>
        {record.status === "accepted" ? (
          <p><strong>Eligible.</strong> RepoDesk may include this record in future bounded context packs when lexical relevance and the knowledge budget allow it.</p>
        ) : (
          <p><strong>Excluded.</strong> Candidate and archived records never enter agent context.</p>
        )}
      </section>

      <footer className="knowledge-detail-actions">
        {record.status === "candidate" ? (
          <button type="button" className="primary-button" disabled={busy} onClick={onAccept}>
            Accept into project knowledge
          </button>
        ) : null}
        {record.status !== "archived" ? (
          <button type="button" className="ghost-button" disabled={busy} onClick={onArchive}>
            {record.status === "candidate" ? "Reject / archive" : "Archive"}
          </button>
        ) : null}
      </footer>
    </article>
  );
}

function ProposalForm({
  busy,
  onSubmit,
  onCancel,
}: {
  busy: boolean;
  onSubmit: (input: { category: EngineeringKnowledgeCategory; title: string; content: string }) => void;
  onCancel: () => void;
}) {
  const [category, setCategory] = useState<EngineeringKnowledgeCategory>("convention");
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const canSubmit = title.trim().length > 0 && content.trim().length > 0;

  return (
    <section className="knowledge-proposal-form">
      <div className="knowledge-proposal-grid">
        <label>
          <span>Category</span>
          <select value={category} onChange={(event) => setCategory(event.target.value as EngineeringKnowledgeCategory)}>
            {(Object.keys(CATEGORY_LABELS) as EngineeringKnowledgeCategory[]).map((value) => (
              <option key={value} value={value}>{CATEGORY_LABELS[value]}</option>
            ))}
          </select>
        </label>
        <label>
          <span>Title</span>
          <input value={title} onChange={(event) => setTitle(event.target.value)} placeholder="Short engineering rule" />
        </label>
      </div>
      <label>
        <span>Content</span>
        <textarea
          rows={5}
          value={content}
          onChange={(event) => setContent(event.target.value)}
          placeholder="Describe the reusable architecture rule, hazard, decision, command, or convention."
        />
      </label>
      <div className="knowledge-proposal-actions">
        <button
          type="button"
          className="primary-button"
          disabled={busy || !canSubmit}
          onClick={() => onSubmit({ category, title: title.trim(), content: content.trim() })}
        >
          Propose candidate
        </button>
        <button type="button" className="ghost-button" disabled={busy} onClick={onCancel}>Cancel</button>
      </div>
    </section>
  );
}

export function KnowledgeTab() {
  const { hasProject } = useWorkspace();
  const queryClient = useQueryClient();
  const [filter, setFilter] = useState<Filter>("review");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [showProposal, setShowProposal] = useState(false);

  const snapshot = useQuery({
    queryKey: ENGINEERING_KNOWLEDGE_KEY,
    queryFn: engineeringKnowledgeSnapshot,
    enabled: hasProject,
    refetchInterval: 8_000,
  });

  const propose = useMutation({
    mutationFn: proposeEngineeringKnowledge,
    onSuccess: (next) => {
      applySnapshot(queryClient, next);
      setShowProposal(false);
      const newest = next.records.find((record) => record.status === "candidate");
      if (newest) setSelectedId(newest.id);
    },
  });
  const capture = useMutation({
    mutationFn: captureVerifiedKnowledgeCommand,
    onSuccess: (next) => applySnapshot(queryClient, next),
  });
  const accept = useMutation({
    mutationFn: acceptEngineeringKnowledge,
    onSuccess: (next) => applySnapshot(queryClient, next),
  });
  const archive = useMutation({
    mutationFn: archiveEngineeringKnowledge,
    onSuccess: (next) => applySnapshot(queryClient, next),
  });

  const data = snapshot.data;
  const records = useMemo(() => {
    const items = data?.records ?? [];
    switch (filter) {
      case "review": return items.filter((record) => record.status === "candidate");
      case "accepted": return items.filter((record) => record.status === "accepted");
      case "archived": return items.filter((record) => record.status === "archived");
      case "all": return items;
    }
  }, [data?.records, filter]);

  useEffect(() => {
    if (records.length === 0) {
      setSelectedId(null);
      return;
    }
    if (!selectedId || !records.some((record) => record.id === selectedId)) {
      setSelectedId(records[0].id);
    }
  }, [records, selectedId]);

  if (!hasProject) {
    return <div className="focus-empty">Connect a project before creating Engineering Knowledge.</div>;
  }
  if (snapshot.isLoading || !data) {
    return <div className="focus-empty">Loading Project Knowledge…</div>;
  }
  if (snapshot.isError) {
    return <div className="notice danger">{errorToMessage(snapshot.error)}</div>;
  }

  const selected = data.records.find((record) => record.id === selectedId) ?? null;
  const busy = propose.isPending || capture.isPending || accept.isPending || archive.isPending;
  const mutationError = propose.error ?? capture.error ?? accept.error ?? archive.error;

  return (
    <div className="knowledge-workspace">
      <header className="focus-page-header">
        <div>
          <p className="eyebrow">Project Knowledge</p>
          <h1>Reviewed engineering memory</h1>
          <p className="muted">Only accepted records may enter future context packs. Candidates remain visible for human review.</p>
        </div>
        <button type="button" className="primary-button" onClick={() => setShowProposal(true)}>New candidate</button>
      </header>

      <div className="knowledge-summary-strip">
        <button className={filter === "review" ? "active" : ""} onClick={() => setFilter("review")}>
          <strong>{data.counts.candidates}</strong><span>To review</span>
        </button>
        <button className={filter === "accepted" ? "active" : ""} onClick={() => setFilter("accepted")}>
          <strong>{data.counts.accepted}</strong><span>Accepted</span>
        </button>
        <button className={filter === "archived" ? "active" : ""} onClick={() => setFilter("archived")}>
          <strong>{data.counts.archived}</strong><span>Archived</span>
        </button>
        <button className={filter === "all" ? "active" : ""} onClick={() => setFilter("all")}>
          <strong>{data.records.length}</strong><span>All</span>
        </button>
      </div>

      {data.suggestions.length > 0 ? (
        <section className="knowledge-suggestions" aria-label="Evidence-backed suggestions">
          <div className="knowledge-section-heading">
            <div>
              <strong>Fresh verification can teach the project</strong>
              <span>Successful commands are suggestions only until you capture and accept them.</span>
            </div>
            <span className="pill">{data.suggestions.length}</span>
          </div>
          {data.suggestions.map((suggestion) => (
            <div className="knowledge-suggestion-row" key={suggestion.suggestion_id}>
              <div>
                <strong>{suggestion.content}</strong>
                <small>Verified in {suggestion.source_work_item_id}</small>
              </div>
              <button
                type="button"
                className="tiny-button"
                disabled={busy}
                onClick={() => capture.mutate(suggestion.content)}
              >
                Capture candidate
              </button>
            </div>
          ))}
        </section>
      ) : null}

      {showProposal ? (
        <ProposalForm busy={busy} onSubmit={(input) => propose.mutate(input)} onCancel={() => setShowProposal(false)} />
      ) : null}

      {mutationError ? <div className="notice danger">{errorToMessage(mutationError)}</div> : null}

      <div className="knowledge-master-detail">
        <aside className="knowledge-list" aria-label="Engineering Knowledge records">
          <div className="knowledge-list-heading">
            <strong>{filter === "review" ? "Needs review" : filter === "accepted" ? "Accepted knowledge" : filter === "archived" ? "Archive" : "All knowledge"}</strong>
            <span>{records.length}</span>
          </div>
          {records.length === 0 ? (
            <p className="focus-empty compact">Nothing in this view.</p>
          ) : records.map((record) => (
            <KnowledgeListItem
              key={record.id}
              record={record}
              selected={record.id === selectedId}
              onSelect={() => setSelectedId(record.id)}
            />
          ))}
        </aside>

        {selected ? (
          <KnowledgeDetail
            record={selected}
            busy={busy}
            onAccept={() => accept.mutate(selected.id)}
            onArchive={() => archive.mutate(selected.id)}
          />
        ) : (
          <div className="knowledge-detail focus-empty">Select a knowledge record.</div>
        )}
      </div>
    </div>
  );
}
