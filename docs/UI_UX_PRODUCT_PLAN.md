# RepoDesk UI/UX Product Plan

RepoDesk should feel like a guided cockpit, not a raw debug dashboard.

## Current UI principles

1. One primary action: Do next safe step.
2. Always show whether project, task, Git, and next action are ready.
3. Never hide failures: every Tauri command appears in Debug.
4. Make Git status visible before AI or patch actions.
5. Keep artifacts easy to inspect and copy.
6. Use loading overlays and toasts for every long action.

## Near-term UX improvements

### Product workflow hardening

- Better empty states when project/task is missing.
- Ready-to-commit checklist.
- Before/after Git state after running actions.
- Explain why an action is recommended.
- Show expected output before running an action.

### Visual polish

- Consistent card sizes.
- Better responsive behavior.
- Better content width limits.
- Status strip at the top of every tab.
- More readable artifact/prompt viewer.

### Debug experience

- Export debug bundle from UI.
- Copy debug event.
- Filter Debug by success/error.
- Keep last 100 command events.

### Security UX

- Show whether an action is read-only, safe, guarded, expensive, or blocked.
- Warn when Git workspace is dirty.
- Never expose unrestricted shell from UI.
