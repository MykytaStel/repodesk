import { useQuery } from "@tanstack/react-query";
import { queryKeys, optionalCommand } from "../../shared/api/queries";
import { getString, gitIsDirty, gitDirtyCount } from "../../shared/utils/helpers";

export function useGit() {
  const { data: git, isLoading } = useQuery({
    queryKey: queryKeys.git.snapshot,
    queryFn: () => optionalCommand<unknown>("git_workspace_snapshot"),
  });

  const branch = getString(git, "branch", getString(git, "current_branch", "-"));
  const dirty = gitIsDirty(git);
  const dirtyCount = gitDirtyCount(git);

  return {
    git,
    branch,
    dirty,
    dirtyCount,
    isLoading,
  };
}
