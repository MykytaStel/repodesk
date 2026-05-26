# RepoDesk Desktop MVP

Local-first Tauri 2 desktop shell for RepoDesk.

## Run

```bash
./scripts/dev-desktop.sh
```

## Verify

```bash
./scripts/verify-desktop-mvp.sh
```

## Security posture

- Tauri window loads only `127.0.0.1` Vite dev server during development.
- CSP is enabled in `tauri.conf.json`.
- The desktop app exposes only read-only commands at this stage:
  - `dashboard_snapshot`
  - `security_audit_text`
  - `local_state_status`
- No unrestricted shell command execution is exposed to the UI.
- No secrets are read by the UI.

## Next increments

- Add SQLite-backed session history to the desktop app.
- Add button actions behind explicit guard/judge checks.
- Add settings screen for providers and budget limits.
- Add event stream from core to UI.
