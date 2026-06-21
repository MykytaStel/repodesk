import { useMemo, useState } from "react";
import { useMemory } from "./useMemory";
import { AddMemoryPanel } from "./AddMemoryPanel";
import { BrainPreviewPanel } from "./BrainPreviewPanel";
import { CapturePanel } from "./CapturePanel";
import { EntryList } from "./EntryList";
import { MemoryHero, NoProjectMemoryState } from "./MemoryHero";
import { MemoryMetrics } from "./MemoryMetrics";
import { ReviewQueue } from "./ReviewQueue";
import { type Category, type EntryStatusFilter } from "./constants";
import { buildEntriesById, filterEntries, getMemoryStats, parseTags, type MemoryStats } from "./utils";
import type { BrainPreview } from "../../shared/api/memory";

export function MemoryTab() {
  const memory = useMemory();
  const [content, setContent] = useState("");
  const [category, setCategory] = useState<Category>("decision");
  const [tagsRaw, setTagsRaw] = useState("");
  const [captureAgent, setCaptureAgent] = useState("chatgpt");
  const [captureText, setCaptureText] = useState("");
  const [search, setSearch] = useState("");
  const [filterCategory, setFilterCategory] = useState<Category | "all">("all");
  const [filterStatus, setFilterStatus] = useState<EntryStatusFilter>("active");

  const stats = useMemo(
    () => getMemoryStats(memory.entries, memory.proposals),
    [memory.entries, memory.proposals],
  );
  const entriesById = useMemo(() => buildEntriesById(memory.entries), [memory.entries]);
  const visibleEntries = useMemo(
    () =>
      filterEntries(memory.entries, {
        search,
        category: filterCategory,
        status: filterStatus,
      }),
    [filterCategory, filterStatus, memory.entries, search],
  );

  async function handleAdd() {
    const trimmed = content.trim();
    if (!trimmed) return;
    await memory.addEntry.mutateAsync({
      content: trimmed,
      category,
      tags: parseTags(tagsRaw),
    });
    setContent("");
    setTagsRaw("");
  }

  async function handleCapture() {
    const text = captureText.trim();
    if (!text) return;
    await memory.capture.mutateAsync({ agent: captureAgent, text });
    setCaptureText("");
  }

  if (!memory.hasProject) {
    return <NoProjectMemoryState />;
  }

  return (
    <div className="content-grid">
      <MemoryHero
        projectName={memory.projectName}
        scanPending={memory.scan.isPending}
        consolidatePending={memory.consolidate.isPending}
        onScan={() => memory.scan.mutate()}
        onConsolidate={() => memory.consolidate.mutate()}
      />

      <MemoryMetrics stats={stats} />

      <MemoryFlowPanel stats={stats} preview={memory.preview} />

      <BrainPreviewPanel preview={memory.preview} loading={memory.previewLoading} />

      <CapturePanel
        agent={captureAgent}
        text={captureText}
        pending={memory.capture.isPending}
        error={memory.capture.error}
        result={memory.capture.data}
        onAgentChange={setCaptureAgent}
        onTextChange={setCaptureText}
        onCapture={() => void handleCapture()}
      />

      <ReviewQueue
        proposals={memory.proposals}
        entriesById={entriesById}
        pendingCount={stats.pendingCount}
        busy={memory.accept.isPending || memory.reject.isPending || memory.reconcile.isPending}
        reconcileError={memory.reconcile.error}
        onAccept={(id, keepId) => memory.accept.mutate({ id, keepId })}
        onReject={(id) => memory.reject.mutate(id)}
        onReconcile={(id) => memory.reconcile.mutate(id)}
      />

      <AddMemoryPanel
        content={content}
        category={category}
        tagsRaw={tagsRaw}
        pending={memory.addEntry.isPending}
        onContentChange={setContent}
        onCategoryChange={setCategory}
        onTagsChange={setTagsRaw}
        onAdd={() => void handleAdd()}
      />

      <EntryList
        entries={visibleEntries}
        loading={memory.entriesLoading}
        search={search}
        category={filterCategory}
        status={filterStatus}
        onSearchChange={setSearch}
        onCategoryChange={setFilterCategory}
        onStatusChange={setFilterStatus}
        onPin={(id, pinned) => memory.setPinned.mutate({ id, pinned })}
        onArchive={(entry) =>
          memory.setStatus.mutate({
            id: entry.id,
            status: entry.status === "archived" ? "active" : "archived",
          })
        }
        onDelete={(id) => memory.deleteEntry.mutate(id)}
        onSaveEdit={(id, nextContent, nextCategory, tags) =>
          memory.updateEntry.mutate({
            id,
            content: nextContent,
            category: nextCategory,
            tags,
          })
        }
      />
    </div>
  );
}

function MemoryFlowPanel({
  stats,
  preview,
}: {
  stats: MemoryStats;
  preview: BrainPreview | null;
}) {
  const included = preview ? `${preview.included}/${preview.total_active}` : "0/0";
  const dropped = preview && preview.excluded > 0 ? `${preview.excluded} dropped by budget` : "within budget";
  return (
    <section className="panel wide-panel memory-flow">
      <div className="panel-title-row compact">
        <div>
          <p className="eyebrow">Memory pipeline</p>
          <h2>What becomes agent context</h2>
        </div>
        <span className="pill">{included} injected</span>
      </div>
      <div className="memory-flow-grid">
        <MemoryFlowStep
          label="Sources"
          value={`${stats.activeCount} active entries`}
          detail="Manual notes, accepted proposals, pinned decisions, and prior run captures."
        />
        <MemoryFlowStep
          label="Review queue"
          value={`${stats.pendingCount} pending`}
          detail="Captured text waits here until you accept, reject, or reconcile it."
        />
        <MemoryFlowStep
          label="Brain file"
          value={`${stats.pinnedCount} pinned`}
          detail="Rebuild memory.md writes the curated project memory file."
        />
        <MemoryFlowStep
          label="Agent slice"
          value={dropped}
          detail="Pinned, relevant, and recent entries are packed into the context sent to agents."
        />
      </div>
    </section>
  );
}

function MemoryFlowStep({
  label,
  value,
  detail,
}: {
  label: string;
  value: string;
  detail: string;
}) {
  return (
    <div className="memory-flow-step">
      <span>{label}</span>
      <strong>{value}</strong>
      <p>{detail}</p>
    </div>
  );
}
