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
import { buildEntriesById, filterEntries, getMemoryStats, parseTags } from "./utils";

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
