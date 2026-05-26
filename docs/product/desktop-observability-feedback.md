# Desktop Observability and Feedback Pack

Recommended branch: `feature/desktop-observability-feedback`

This pack makes the desktop UI understandable during real use.

## Adds

- Toast notifications for success/error/info.
- Loading states for every major action.
- Debug console that records every Tauri command invocation.
- AI Discovery screen that clearly shows found/missing tools and endpoints.
- Artifact viewer for prompts, contexts and checks summaries.
- Action result panel with stdout/stderr/exit code.
- UI micro-animations and status badges.

## Security posture

- No unrestricted shell access is added.
- The UI still invokes only existing Tauri commands.
- Missing backend commands are shown as explicit debug errors instead of failing silently.
- Debug output is local-only and not sent anywhere.

## Product goal

The user should always understand:

1. Is RepoDesk working?
2. What command was called?
3. Did it succeed?
4. What did it find?
5. Where can I inspect debug output?

