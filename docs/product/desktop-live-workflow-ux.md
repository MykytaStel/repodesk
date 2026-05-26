# Desktop Live Workflow UX Pack

Recommended branch: `feature/desktop-live-workflow-ux`

This pack turns the desktop UI from static/debug panels into a more understandable live cockpit.

## Adds

- Startup loader with clear boot phases.
- Setup panel for connecting/activating a project and creating a task.
- Dynamic workflow timeline with a single primary action.
- Toast notifications for success, warning, and errors.
- Loading overlay for long-running actions.
- AI discovery cards showing found/missing tools and local endpoints.
- Debug console with command duration, arguments, result preview, and errors.
- Artifact browser for prompts/context/check summaries.
- Last result panel with status/output/error.
- Auto-refresh after actions.
- Local UI state persistence for the selected tab.

## Security posture

- The UI still cannot run arbitrary shell commands.
- Project/task operations go through existing safe Tauri commands.
- AI discovery remains passive: PATH lookup, known app lookup, and localhost checks.
- Debug output is local-only.

## Product goal

The user should always understand:

1. What RepoDesk is doing now.
2. Whether it succeeded or failed.
3. What was found during AI discovery.
4. Which step is recommended next.
5. Where to debug failures.
