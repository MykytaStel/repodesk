# RepoDesk Development Roadmap

RepoDesk is a local-first desktop control cockpit for AI-assisted development.
The core product idea is simple: connect projects, create tasks, build safe context, route work to the right AI/runtime, run checks, and keep the workflow observable and secure.

## Current product goal

Make RepoDesk useful for daily work before adding more advanced AI automation.

The minimum daily workflow should be:

1. Open the desktop app.
2. Connect or select a project.
3. Create or select a task.
4. See Git workspace status.
5. Build smart context.
6. Run safety/budget checks.
7. Generate or copy prompts.
8. Run project checks.
9. Review action history and debug output.
10. Commit/push safely when the workspace is ready.

## Near-term milestones

### Milestone 1 — Stabilize current desktop MVP

Status target: all local verification scripts pass.

Scope:
- Keep desktop commands bounded and explicit.
- Make loading/error states visible.
- Add debug bundle export.
- Add basic secret scan.
- Add local health report.
- Keep UI focused on product workflow, not raw panels only.

### Milestone 2 — Git-aware workflow

Scope:
- Show branch, last commit, dirty state, staged/unstaged/untracked files.
- Warn before actions when the workspace is dirty.
- Show what RepoDesk changed after actions.
- Add a “ready to commit” checklist.

### Milestone 3 — SQLite state unification

Scope:
- Move action history, sessions, settings, provider preferences, and task state into SQLite.
- Keep local files for artifacts: context.md, smart-context.md, prompts, checks-summary.md.
- Add migration/version table.

### Milestone 4 — Provider runtime health

Scope:
- Detect Ollama/LM Studio/local tools.
- Query Ollama /api/tags.
- Show model list and model health.
- Route compression/review tasks to local models when possible.
- Keep paid providers disabled unless explicitly allowed.

### Milestone 5 — Desktop packaging

Scope:
- Build a local desktop app with Tauri.
- Add app icon set.
- Add release checklist.
- Add backup/restore for local state.

## Development priorities

1. Stability before new features.
2. Observability before automation.
3. Security before agent execution.
4. Git visibility before patching.
5. SQLite persistence before advanced UI.
6. Local AI first, paid AI guarded.

## Branch naming

Use feature branches by capability:

- feature/stability-optimization
- feature/git-workspace-awareness
- feature/sqlite-state-store
- feature/ollama-health-runtime
- feature/desktop-packaging
- feature/security-hardening

Use fix branches for targeted errors:

- fix/desktop-build
- fix/tauri-command-namespace
- fix/typescript-build

## Commit style

Use clear product-focused commits:

- Add desktop workflow health checks
- Add git workspace visibility
- Add SQLite state store
- Add Ollama runtime health checks
- Harden desktop command allowlist
