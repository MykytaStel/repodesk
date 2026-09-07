# RepoDesk UI/UX Product Plan

RepoDesk should feel like a calm decision cockpit, not a raw debug dashboard or an
agent chat wrapper. The primary screen is the active Work Item and the next evidence
decision that moves it safely forward.

## Current UI principles

1. One primary action: Do next safe step.
2. Always show whether project, task, Git, and next action are ready.
3. Never hide failures: every Tauri command appears in Debug.
4. Make Git status visible before AI or patch actions.
5. Keep artifacts easy to inspect and copy.
6. Use loading overlays and toasts for every long action.
7. Prefer `Unknown` or `Not measured` to invented confidence.
8. Show the cost and verification debt of a decision before it becomes invisible.

## Near-term UX improvements

### Product workflow hardening

- Better empty states when project/task is missing.
- Ready-to-commit checklist.
- Before/after Git state after running actions.
- Explain why an action is recommended.
- Show expected output before running an action.
- Make Adaptive Verification recommendation-first: run targeted, defer with debt,
  ask for approval, or stop with a partial result.
- Record the accepted choice as a Decision Receipt with tree, policy and evidence
  identity; the UI must not run checks implicitly while recording that choice.

### Visual polish

- Consistent card sizes.
- Better responsive behavior.
- Better content width limits.
- Work Item control header with one dominant next action and compact Run Control,
  Change Economics and Decision Receipt evidence cards.
- Runs subviews named by the engineering job: Run timeline, Change economics,
  Evidence archive.
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
