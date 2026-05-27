import { useQuery } from "@tanstack/react-query";
import { queryKeys, optionalCommand } from "../../shared/api/queries";
import { codeChangedFiles } from "../../shared/utils/helpers";

export function useCode() {
  const { data: codeWorkbench, isLoading } = useQuery({
    queryKey: queryKeys.code.workbench,
    queryFn: () => optionalCommand<unknown>("code_workbench_snapshot"),
  });

  const changedFiles = codeChangedFiles(codeWorkbench);

  return { codeWorkbench, changedFiles, isLoading };
}
