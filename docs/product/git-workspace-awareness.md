# Git Workspace Awareness Pack

Recommended branch: `feature/git-workspace-awareness`

This pack makes RepoDesk aware of the active project's Git working tree so the desktop UI can show what changed before and after AI-assisted actions.

## Adds

- Core Git workspace snapshot module.
- Active project Git status detection.
- Branch and last commit display.
- Staged / unstaged / untracked grouping.
- Diff stat and raw porcelain status.
- Dirty workspace warnings before guarded actions.
- Desktop command: `git_workspace_snapshot`.
- Git tab in the desktop UI.
- Debug visibility for refresh/scans/actions.
- Verification script.

## Security posture

- Read-only Git commands only.
- No commit, push, reset, checkout, or file modification commands.
- Uses the active project path from RepoDesk project config.
- Does not read secrets or full file contents.

## Product goal

Before an AI agent changes anything, RepoDesk should show:

1. Which branch is active.
2. Whether the workspace is dirty.
3. Which files are staged, unstaged, and untracked.
4. What changed after an action.
5. Whether it is safe to continue or better to commit first.
