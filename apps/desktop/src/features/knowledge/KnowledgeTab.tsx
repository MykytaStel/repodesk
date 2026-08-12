import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ENGINEERING_KNOWLEDGE_KEY,
  acceptEngineeringKnowledge,
  archiveEngineeringKnowledge,
  captureVerifiedKnowledgeCommand,
  engineeringKnowledgeSnapshot,
  proposeEngineeringKnowledge,
  reconfirmEngineeringKnowledge,
  type EngineeringKnowledgeCategory,
  type EngineeringKnowledgeLifecycleEntry,
  type EngineeringKnowledgeLifecycleState,
  type EngineeringKnowledgeRecord,
  type EngineeringKnowledgeSnapshot,
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

const LIFECYCLE_LABELS: Record<EngineeringKnowledgeLifecycleState, string> = {
  pending_review: "Pending review",
  current: "Current",
  review_soon: "Review soon",
  review_required: "Review required",
  archived: "Archived",
};

type Filter = "review" | "accepted" | "archived" | "all";

const EMPTY_COPY: Record<Filter, { title: string; detail: string }> = {
  review: {
    title: "Nothing needs review",
    detail: "New candidates and accepted knowledge approaching its review boundary will appear here.",
  },
  accepted: {
    title: "No accepted knowledge",
    detail: "Accepted records remain reusable only while their human review lifecycle is current.",
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

function toneForLifecycle(state: EngineeringKnowledgeLifecycleState): string {
  if (state === "current") return "ok";
  if (state === "review_required") return "danger";
  if (state === "review_soon" || state === "pending_review") return "warn";
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
  lifecycle,
  selected,
  onSelect,
}: {
  record: EngineeringKnowledgeRecord;
  lifecycle: EngineeringKnowledgeLifecycleEntry;
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
        <span className={`pill ${toneForLifecycle(lifecycle.state)}`}>
          {LIFECYCLE_LABELS[lifecycle.state]}
        </span>
      </div>
      <strong>{record.title}</strong>
      <small>
        {record.origin === "verification" ? "Verified evidence" : "Human proposed"}
        {lifecycle.review_due_at ? ` · review ${new Date(lifecycle.review_due_at).toLocaleDateString()}` : ""}
      </small>
    </button>
  );
}

function KnowledgeDetail({
  record,
  lifecycle,
  busy,
  onAccept,
  onReconfirm,
  onArchive,
}: {
  record: EngineeringKnowledgeRecord;
  lifecycle: EngineeringKnowledgeLifecycleEntry;
  busy: boolean;
  onAccept: () => void;
  onReconfirm: () => void;
  onArchive: () => void;
}) {
  const contextState = (() => {
    switch (lifecycle.state) {
      case "current":
        return <p><strong>Eligible.</strong> RepoDesk may reuse this when relevant and within context budget.</p>;
      case "review_soon":
        return <p><strong>Eligible, review soon.</strong> It can still enter context, but its review boundary is approaching.</p>;
      case "review_required":
        return <p><strong>Excluded until reconfirmed.</strong> RepoDesk fails closed and will not inject this into agent context.</p>;
      case "pending_review":
        return <p><strong>Excluded.</strong> New knowledge requires explicit human acceptance first.</p>;
      case "archived":
        return <p><strong>Excluded.</strong> Archived records remain only for auditability.</p>;
    }
  })();

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
        <span className={`pill ${toneForLifecycle(lifecycle.state)}`}>
          {LIFECYCLE_LABELS[lifecycle.state]}
        </span>
      </header>

      <section className="knowledge-detail-section">
        <span className="knowledge-section-label">Knowledge</span>
        <p className="knowledge-content">{record.content}</p>
      </section>

      <section className="knowledge-detail-section">
        <span className="knowledge-section-label">Review lifecycle</span>
        <div className="knowledge-provenance-grid">
          <span>State</span><strong>{LIFECYCLE_LABELS[lifecycle.state]}</strong>
          <span>Age</span><strong>{lifecycle.age_days} days</strong>
          <span>Review cadence</span><strong>{lifecycle.review_after_days ? `${lifecycle.review_after_days} days` : "Not applicable"}</strong>
          <span>Review due</span><strong>{lifecycle.review_due_at ? new Date(lifecycle.review_due_at).toLocaleString() : "Not applicable"}</strong>
        </div>
        <p className={lifecycle.state === "review_required" ? "notice warning" : "muted"}>{lifecycle.reason}</p>
      </section>

      <section className="knowledge-detail-section">
        <span className="knowledge-section-label">Provenance</span>
        <div className="knowledge-provenance-grid">
          <span>Project</span><strong>{record.project}</strong>
          <span>Work Item</span><strong>{record.source_work_item_id ?? "Not bound"}</strong>
          <span>Evidence</span><strong>{record.evidence.length}</strong>
          <span>Last human review</span><strong>{new Date(record.updated_at).toLocaleString()}</strong>
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
        {contextState}
      </section>

      <footer className="knowledge-detail-actions">
        {record.status === "candidate" ? (
          <button type="button" className="primary-button" disabled={busy} onClick={onAccept}>
            Accept
          </button>
        ) : null}
        {record.status === "accepted" && lifecycle.state === "review_required" ? (
          <button type="button" className="primary-button" disabled={busy} onClick={onReconfirm}>
            Reconfirm for context
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
  const reconfirm = useMutation({
    mutationFn: reconfirmEngineeringKnowledge,
    onSuccess: (next) => applySnapshot(queryClient, next),
  });
  const archive = useMutation({
    mutationFn: archiveEngineeringKnowledge,
    onSuccess: (next) => applySnapshot(queryClient, next),
  });

  const data = snapshot.data;
  const lifecycleById = useMemo(
    () => new Map((data?.lifecycle.entries ?? []).map((entry) => [entry.knowledge_id, entry])),
    [data?.lifecycle.entries],
  );
  const records = useMemo(() => {
    const items = data?.records ?? [];
    switch (filter) {
      case "review":
        return items.filter((record) => {
          const state = lifecycleById.get(record.id)?.state;
          return state === "pending_review" || state === "review_soon" || state === "review_required";
        });
      case "accepted": return items.filter((record) => record.status === "accepted");
      case "archived": return items.filter((record) => record.status === "archived");
      case "all": return items;
    }
  }, [data?.records, filter, lifecycleById]);

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
  const selectedLifecycle = selected ? lifecycleById.get(selected.id) ?? null : null;
  const busy = propose.isPending || capture.isPending || accept.isPending || reconfirm.isPending || archive.isPending;
  const mutationError = propose.error ?? capture.error ?? accept.error ?? reconfirm.error ?? archive.error;
  const empty = EMPTY_COPY[filter];
  const reviewCount = data.lifecycle.counts.pending_review
    + data.lifecycle.counts.review_soon
    + data.lifecycle.counts.review_required;

  return (
    <div className="knowledge-workspace knowledge-workspace-v2">
      <header className="focus-page-header knowledge-page-header">
        <div>
          <p className="eyebrow">Project Knowledge</p>
          <h1>Engineering knowledge</h1>
          <p className="muted">Reviewed rules, decisions and commands RepoDesk can reuse only while their review lifecycle remains valid.</p>
        </div>
        <button type="button" className="primary-button" onClick={() => setShowProposal(true)}>Add knowledge</button>
      </header>

      <div className="knowledge-summary-strip">
        <button className={filter === "review" ? "active" : ""} onClick={() => setFilter("review")}>
          <strong>{reviewCount}</strong><span>Review</span>
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

      {data.lifecycle.counts.review_required > 0 ? (
        <div className="notice warning" role="status">
          {data.lifecycle.counts.review_required} accepted knowledge record{data.lifecycle.counts.review_required === 1 ? " is" : "s are"} excluded from agent context until reconfirmed.
        </div>
      ) : null}

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
            {records.map((record) => {
              const lifecycle = lifecycleById.get(record.id);
              if (!lifecycle) return null;
              return (
                <KnowledgeListItem
                  key={record.id}
                  record={record}
                  lifecycle={lifecycle}
                  selected={record.id === selectedId}
                  onSelect={() => setSelectedId(record.id)}
                />
              );
            })}
          </aside>

          {selected && selectedLifecycle ? (
            <KnowledgeDetail
              record={selected}
              lifecycle={selectedLifecycle}
              busy={busy}
              onAccept={() => accept.mutate(selected.id)}
              onReconfirm={() => reconfirm.mutate(selected.id)}
              onArchive={() => archive.mutate(selected.id)}
            />
          ) : null}
        </div>
      )}
    </div>
  );
}
