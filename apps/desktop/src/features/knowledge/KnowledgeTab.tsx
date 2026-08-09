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

const EMPTY_COPY: Record<Filter, { title: string; detail: string }> = {
  review: {
    title: "Nothing to review",
    detail: "Add a durable project rule or capture a verified command when there is something worth reusing.",
  },
  accepted: {
    title: "No accepted knowledge",
    detail: "Accepted records are the only project knowledge RepoDesk may reuse in future context.",
  },
  archived: {
    title: "Archive is empty",
    detail: "Retired project knowledge stays here for auditability.",
  },
  all: {
    title: "No project knowledge yet",
    detail: "Keep only durable rules, decisions, hazards, commands and testing conventions here.",
  },
};

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
        {record.origin === "verification" ? "Verified evidence" : "Human proposed"}
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
            {record.origin === "verification" ? "Captured from verified evidence" : "Added by a human"}
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
          <span>Work Item</span><strong>{record.source_work_item_id ?? "Not bound"}</strong>
          <span>Evidence</span><strong>{record.evidence.length}</strong>
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
          <p className="muted">No machine evidence attached.</p>
        )}
      </section>

      <section className="knowledge-detail-section knowledge-context-state">
        <span className="knowledge-section-label">Future context</span>
        {record.status === "accepted" ? (
          <p><strong>Eligible.</strong> RepoDesk may reuse this when it is relevant and fits the context budget.</p>
        ) : (
          <p><strong>Excluded.</strong> Review or archived records never enter agent context.</p>
        )}
      </section>

      <footer className="knowledge-detail-actions">
        {record.status === "candidate" ? (
          <button type="button" className="primary-button" disabled={busy} onClick={onAccept}>
            Accept
          </button>
        ) : null}
        {record.status !== "archived" ? (
          <button type="button" className="ghost-button" disabled={busy} onClick={onArchive}>
            {record.status === "candidate" ? "Reject" : "Archive"}
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
          <input value={title} onChange={(event) => setTitle(event.target.value)} placeholder="Short reusable rule" />
        </label>
      </div>
      <label>
        <span>Content</span>
        <textarea
          rows={5}
          value={content}
          onChange={(event) => setContent(event.target.value)}
          placeholder="What should future work on this project remember?"
        />
      </label>
      <div className="knowledge-proposal-actions">
        <button
          type="button"
          className="primary-button"
          disabled={busy || !canSubmit}
          onClick={() => onSubmit({ category, title: title.trim(), content: content.trim() })}
        >
          Add for review
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
    staleTime: 30_000,
    refetchOnWindowFocus: true,
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
    return <div className="focus-empty">Connect a project to use Project Knowledge.</div>;
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
  const empty = EMPTY_COPY[filter];

  return (
    <div className="knowledge-workspace knowledge-workspace-v2">
      <header className="focus-page-header knowledge-page-header">
        <div>
          <p className="eyebrow">Project Knowledge</p>
          <h1>Engineering knowledge</h1>
          <p className="muted">Reviewed rules, decisions and commands RepoDesk can reuse in future work.</p>
        </div>
        <button type="button" className="primary-button" onClick={() => setShowProposal(true)}>Add knowledge</button>
      </header>

      <div className="knowledge-summary-strip">
        <button className={filter === "review" ? "active" : ""} onClick={() => setFilter("review")}>
          <strong>{data.counts.candidates}</strong><span>Review</span>
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
              <strong>Verified commands ready to review</strong>
              <span>Capture only commands worth reusing in future work.</span>
            </div>
            <span className="pill">{data.suggestions.length}</span>
          </div>
          {data.suggestions.map((suggestion) => (
            <div className="knowledge-suggestion-row" key={suggestion.suggestion_id}>
              <div>
                <strong>{suggestion.content}</strong>
                <small>{suggestion.source_work_item_id}</small>
              </div>
              <button
                type="button"
                className="tiny-button"
                disabled={busy}
                onClick={() => capture.mutate(suggestion.content)}
              >
                Add for review
              </button>
            </div>
          ))}
        </section>
      ) : null}

      {showProposal ? (
        <ProposalForm busy={busy} onSubmit={(input) => propose.mutate(input)} onCancel={() => setShowProposal(false)} />
      ) : null}

      {mutationError ? <div className="notice danger">{errorToMessage(mutationError)}</div> : null}

      {records.length === 0 ? (
        <section className="knowledge-empty-state">
          <strong>{empty.title}</strong>
          <span>{empty.detail}</span>
          {filter === "review" ? (
            <button type="button" className="tiny-button" onClick={() => setShowProposal(true)}>Add knowledge</button>
          ) : null}
        </section>
      ) : (
        <div className="knowledge-master-detail">
          <aside className="knowledge-list" aria-label="Engineering Knowledge records">
            <div className="knowledge-list-heading">
              <strong>{filter === "review" ? "Needs review" : filter === "accepted" ? "Accepted" : filter === "archived" ? "Archived" : "All knowledge"}</strong>
              <span>{records.length}</span>
            </div>
            {records.map((record) => (
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
          ) : null}
        </div>
      )}
    </div>
  );
}
