import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useGit } from "../git/useGit";
import { codeChangedFiles } from "../../shared/utils/helpers";
import {
  type RepoPilotHistory,
  type RepoPilotReport,
  getRepopilotHistory,
  groupByFile,
  runRepopilotReview,
} from "../../shared/api/repopilot";

/** RepoPilot analysis state shared by Changes and compatibility surfaces.
 * Changed files reuse the canonical Git snapshot instead of triggering the old
 * Code-workbench IPC path, avoiding an extra repository/Git read. */
export function useCode() {
  const queryClient = useQueryClient();
  const { git, isLoading } = useGit();

  const { data: history } = useQuery<RepoPilotHistory>({
    queryKey: ["repopilot_history"],
    queryFn: getRepopilotHistory,
  });

  const review = useMutation<RepoPilotReport>({
    mutationFn: runRepopilotReview,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["repopilot_history"] }),
  });

  const report = review.data ?? null;
  const changedFiles = codeChangedFiles(git);

  return {
    changedFiles,
    isLoading,
    report,
    fileFindings: groupByFile(report),
    reviewing: review.isPending,
    runReview: () => review.mutate(),
    trend: history?.points ?? [],
  };
}
