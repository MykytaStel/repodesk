import type { MemoryEntry, MemoryProposal } from "../../shared/api/memory";
import type { Category, EntryStatusFilter } from "./constants";

export type MemoryStats = {
  pendingCount: number;
  conflictCount: number;
  activeCount: number;
  pinnedCount: number;
};

export function parseTags(value: string): string[] {
  return value
    .split(",")
    .map((tag) => tag.trim())
    .filter(Boolean);
}

export function buildEntriesById(entries: MemoryEntry[]): Map<number, MemoryEntry> {
  const map = new Map<number, MemoryEntry>();
  for (const entry of entries) map.set(entry.id, entry);
  return map;
}

export function getMemoryStats(entries: MemoryEntry[], proposals: MemoryProposal[]): MemoryStats {
  return {
    pendingCount: proposals.length,
    conflictCount: proposals.filter((proposal) => proposal.kind === "conflict").length,
    activeCount: entries.filter((entry) => entry.status === "active").length,
    pinnedCount: entries.filter((entry) => entry.pinned).length,
  };
}

export function filterEntries(
  entries: MemoryEntry[],
  filters: {
    search: string;
    category: Category | "all";
    status: EntryStatusFilter;
  },
): MemoryEntry[] {
  const query = filters.search.trim().toLowerCase();
  return entries.filter((entry) => {
    if (filters.status !== "all" && entry.status !== filters.status) return false;
    if (filters.category !== "all" && entry.category !== filters.category) return false;
    if (!query) return true;

    const haystack = `${entry.content} ${entry.category} ${entry.tags.join(" ")} ${entry.agent}`.toLowerCase();
    return haystack.includes(query);
  });
}
