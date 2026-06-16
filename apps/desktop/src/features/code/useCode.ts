import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { queryKeys, optionalCommand } from "../../shared/api/queries";
import { codeChangedFiles } from "../../shared/utils/helpers";
import {
  RepoPilotHistory,
  RepoPilotReport,
  getRepopilotHistory,
  groupByFile,
  runRepopilotReview,
} from "../../shared/api/repopilot";

export function useCode() {
  const queryClient = useQueryClient();

  const { data: codeWorkbench, isLoading } = useQuery({
    queryKey: queryKeys.code.workbench,
    queryFn: () => optionalCommand<unknown>("code_workbench_snapshot"),
  });

  // Trend is read-only history; the review mutation invalidates it after a run.
  const { data: history } = useQuery<RepoPilotHistory>({
    queryKey: ["repopilot_history"],
    queryFn: getRepopilotHistory,
  });

  const review = useMutation<RepoPilotReport>({
    mutationFn: runRepopilotReview,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["repopilot_history"] }),
  });

  const report = review.data ?? null;
  const changedFiles = codeChangedFiles(codeWorkbench);

  return {
    codeWorkbench,
    changedFiles,
    isLoading,
    report,
    fileFindings: groupByFile(report),
    reviewing: review.isPending,
    runReview: () => review.mutate(),
    trend: history?.points ?? [],
  };
}
