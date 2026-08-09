import { useQuery } from "@tanstack/react-query";
import { queryKeys, optionalCommand } from "../api/queries";
import { getNestedString, getString } from "../utils/helpers";

export function useWorkspace(options: { includeDbStatus?: boolean } = {}) {
  const includeDbStatus = options.includeDbStatus === true;

  const snapshot = useQuery({
    queryKey: queryKeys.workspace.snapshot,
    queryFn: () => optionalCommand<unknown>("desktop_snapshot"),
    staleTime: 2_000,
  });

  const dbStatus = useQuery({
    queryKey: queryKeys.workspace.dbStatus,
    queryFn: () => optionalCommand<unknown>("db_status"),
    enabled: includeDbStatus,
    staleTime: 30_000,
  });

  const projectConfig = useQuery({
    queryKey: queryKeys.workspace.activeProject,
    queryFn: () => optionalCommand<any>("get_active_project_config"),
    staleTime: 5_000,
  });

  const projectName = getString(
    projectConfig.data,
    "name",
    getNestedString(
      snapshot.data,
      ["project", "name"],
      getString(snapshot.data, "project_name", "No active project"),
    ),
  );
  const taskTitle = getNestedString(snapshot.data, ["task", "title"], getString(snapshot.data, "task_title", "No active task"));
  const hasProject = projectName !== "No active project" && projectName !== "-";
  const hasTask = taskTitle !== "No active task" && taskTitle !== "-";

  return {
    snapshot: snapshot.data,
    dbState: dbStatus.data,
    projectConfig: projectConfig.data,
    projectName,
    taskTitle,
    hasProject,
    hasTask,
    isLoading: snapshot.isLoading || projectConfig.isLoading || (includeDbStatus && dbStatus.isLoading),
  };
}
