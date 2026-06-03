import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "../../shared/api/queries";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import * as api from "../../shared/api/memory";

/**
 * Memory Brain hook: entries + proposal queue + "what the AI sees" preview,
 * plus every mutation. All mutations refresh entries, proposals, and preview so
 * the cockpit stays consistent.
 */
export function useMemory() {
  const queryClient = useQueryClient();
  const { projectName, hasProject } = useWorkspace();

  const invalidateAll = () => {
    queryClient.invalidateQueries({ queryKey: queryKeys.memory.list(projectName) });
    queryClient.invalidateQueries({ queryKey: queryKeys.memory.proposals(projectName) });
    queryClient.invalidateQueries({ queryKey: queryKeys.memory.preview(projectName) });
  };

  const entries = useQuery({
    queryKey: queryKeys.memory.list(projectName),
    queryFn: () => (hasProject ? api.readProjectMemory(projectName) : Promise.resolve([])),
    enabled: hasProject,
  });

  const proposals = useQuery({
    queryKey: queryKeys.memory.proposals(projectName),
    queryFn: () => (hasProject ? api.listMemoryProposals(projectName, false) : Promise.resolve([])),
    enabled: hasProject,
  });

  const preview = useQuery({
    queryKey: queryKeys.memory.preview(projectName),
    queryFn: () => (hasProject ? api.memoryBrainPreview(projectName) : Promise.resolve(null)),
    enabled: hasProject,
  });

  const addEntry = useMutation({
    mutationFn: (v: { content: string; category: string; tags: string[] }) =>
      api.appendProjectMemory(projectName, v.content, v.category, v.tags),
    onSuccess: invalidateAll,
  });

  const updateEntry = useMutation({
    mutationFn: (v: { id: number; content: string; category: string; tags: string[] }) =>
      api.updateMemoryEntry(v.id, v.content, v.category, v.tags),
    onSuccess: invalidateAll,
  });

  const deleteEntry = useMutation({
    mutationFn: (id: number) => api.deleteMemoryEntry(id),
    onSuccess: invalidateAll,
  });

  const setPinned = useMutation({
    mutationFn: (v: { id: number; pinned: boolean }) => api.setMemoryPinned(v.id, v.pinned),
    onSuccess: invalidateAll,
  });

  const setStatus = useMutation({
    mutationFn: (v: { id: number; status: string }) => api.setMemoryStatus(v.id, v.status),
    onSuccess: invalidateAll,
  });

  const capture = useMutation({
    mutationFn: (v: { agent: string; text: string }) =>
      api.captureMemory(projectName, v.agent, v.text),
    onSuccess: invalidateAll,
  });

  const scan = useMutation({
    mutationFn: () => api.scanMemory(projectName),
    onSuccess: invalidateAll,
  });

  const accept = useMutation({
    mutationFn: (v: { id: number; keepId?: number | null }) =>
      api.acceptMemoryProposal(v.id, v.keepId ?? null),
    onSuccess: invalidateAll,
  });

  const reject = useMutation({
    mutationFn: (id: number) => api.rejectMemoryProposal(id),
    onSuccess: invalidateAll,
  });

  const reconcile = useMutation({
    mutationFn: (id: number) => api.reconcileMemoryConflict(id),
    onSuccess: invalidateAll,
  });

  const consolidate = useMutation({
    mutationFn: () => api.consolidateMemory(projectName),
    onSuccess: invalidateAll,
  });

  return {
    projectName,
    hasProject,
    entries: entries.data ?? [],
    entriesLoading: entries.isLoading,
    proposals: proposals.data ?? [],
    proposalsLoading: proposals.isLoading,
    preview: preview.data ?? null,
    previewLoading: preview.isLoading,
    addEntry,
    updateEntry,
    deleteEntry,
    setPinned,
    setStatus,
    capture,
    scan,
    accept,
    reject,
    reconcile,
    consolidate,
  };
}
