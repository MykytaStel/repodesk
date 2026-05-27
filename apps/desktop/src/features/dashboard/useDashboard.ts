import { useQuery } from "@tanstack/react-query";
import { queryKeys, optionalCommand } from "../../shared/api/queries";

export function useDashboard() {
  return useQuery({
    queryKey: queryKeys.workspace.snapshot,
    queryFn: () => optionalCommand<unknown>("desktop_snapshot"),
  });
}
