import { invoke } from "@tauri-apps/api/core";

export type MemoryEntry = {
  id: number;
  timestamp: string;
  project: string;
  content: string;
  category: string;
  tags: string[];
  source: string;
  agent: string;
  task_id: string;
  status: string;
  pinned: boolean;
  salience: number;
  confidence: number;
  supersedes_id?: number | null;
  content_hash: string;
  updated_at?: string | null;
};

export type ProposedEntry = {
  content: string;
  category: string;
  tags: string[];
  source: string;
  agent: string;
};

export type ProposalPayload = {
  rationale: string;
  agent: string;
  source_ids: number[];
  proposed?: ProposedEntry | null;
};

export type MemoryProposal = {
  id: number;
  created_at: string;
  project: string;
  task_id: string;
  kind: "capture" | "dedup" | "merge" | "conflict" | string;
  status: "pending" | "accepted" | "rejected" | string;
  payload: ProposalPayload;
  applied_entry_id?: number | null;
};

export type ScanSummary = {
  dedup: number;
  merge: number;
  conflict: number;
  created: MemoryProposal[];
};

export type BrainPreview = {
  markdown: string;
  estimated_tokens: number;
  included: number;
  excluded: number;
  total_active: number;
  pending_proposals: number;
};

export async function readProjectMemory(project: string): Promise<MemoryEntry[]> {
  return invoke("memory_list", { project });
}

export async function appendProjectMemory(
  project: string,
  content: string,
  category: string = "general",
  tags: string[] = [],
): Promise<MemoryEntry> {
  return invoke("memory_add", { project, content, category, tags });
}

export async function searchProjectMemory(project: string, query: string): Promise<MemoryEntry[]> {
  return invoke("memory_search", { project, query });
}

export async function updateMemoryEntry(
  id: number,
  content: string,
  category: string,
  tags: string[],
): Promise<MemoryEntry> {
  return invoke("memory_update", { id, content, category, tags });
}

export async function deleteMemoryEntry(id: number): Promise<void> {
  return invoke("memory_delete", { id });
}

export async function setMemoryPinned(id: number, pinned: boolean): Promise<void> {
  return invoke("memory_set_pinned", { id, pinned });
}

export async function setMemoryStatus(id: number, status: string): Promise<void> {
  return invoke("memory_set_status", { id, status });
}

export async function consolidateMemory(project: string): Promise<string> {
  return invoke("memory_consolidate", { project });
}

export async function memoryBrainPreview(project: string): Promise<BrainPreview> {
  return invoke("memory_brain_preview", { project });
}

export async function captureMemory(project: string, agent: string, text: string): Promise<MemoryProposal[]> {
  return invoke("memory_capture", { project, agent, text });
}

export async function scanMemory(project: string): Promise<ScanSummary> {
  return invoke("memory_scan", { project });
}

export async function listMemoryProposals(project: string, all: boolean = false): Promise<MemoryProposal[]> {
  return invoke("memory_proposals_list", { project, all });
}

export async function acceptMemoryProposal(id: number, keepId?: number | null): Promise<MemoryProposal> {
  return invoke("memory_proposal_accept", { id, keepId: keepId ?? null });
}

export async function rejectMemoryProposal(id: number): Promise<MemoryProposal> {
  return invoke("memory_proposal_reject", { id });
}

export async function reconcileMemoryConflict(id: number): Promise<MemoryProposal> {
  return invoke("memory_reconcile_conflict", { id });
}
