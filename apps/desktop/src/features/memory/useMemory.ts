import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { queryKeys, callCommand, optionalCommand } from "../../shared/api/queries";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import type { MemoryEntry } from "../../shared/api/api";

export function useMemory() {
  const queryClient = useQueryClient();
  const { projectName, hasProject } = useWorkspace();

  const entries = useQuery({
    queryKey: queryKeys.memory.list(projectName),
    queryFn: () =>
      hasProject
        ? optionalCommand<MemoryEntry[]>("memory_list", { project: projectName }).then(
            (res) => res ?? []
          )
        : Promise.resolve([]),
    enabled: hasProject,
  });

  const addMutation = useMutation({
    mutationFn: async ({
      content,
      category,
      tags,
    }: {
      content: string;
      category: string;
      tags: string[];
    }) =>
      callCommand<MemoryEntry>("memory_add", {
        project: projectName,
        content,
        category,
        tags,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.memory.list(projectName) });
    },
  });

  return {
    entries: entries.data ?? [],
    isLoading: entries.isLoading,
    addEntry: addMutation.mutateAsync,
    isAdding: addMutation.isPending,
    addError: addMutation.error as Error | null,
  };
}
