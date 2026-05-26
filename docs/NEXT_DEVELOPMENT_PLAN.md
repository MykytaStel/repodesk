# RepoDesk Next Development Plan

## Immediate objective

Bring the current desktop product into a stable, testable, and understandable state before adding more advanced AI automation.

## Phase 1 — Stabilization and optimization

Branch: `feature/stability-optimization`

Goals:
- Make verification predictable.
- Add fast and full check scripts.
- Add debug bundle generation.
- Add basic secret scanning.
- Document the security model.
- Document the current roadmap and checkpoint process.

Acceptance criteria:
- `./scripts/verify-fast.sh` passes.
- `./scripts/health-report.sh` produces `tmp/repodesk-health-report.md`.
- `./scripts/debug-bundle.sh` produces `.repodesk-debug/<timestamp>/`.
- `.gitignore` protects generated output and secrets.

## Phase 2 — Product workflow hardening

Branch: `feature/product-workflow-hardening`

Goals:
- Make the desktop workflow screen the main product surface.
- Show one clear next safe action.
- Improve empty states and blocked states.
- Add “ready to commit” checklist.
- Show changed files before and after actions.

## Phase 3 — SQLite state store

Branch: `feature/sqlite-state-store`

Goals:
- Persist settings, sessions, events, action history, provider preferences.
- Keep context/prompts/checks as local artifact files.
- Add migrations and DB status UI.

## Phase 4 — Ollama health runtime

Branch: `feature/ollama-health-runtime`

Goals:
- Detect Ollama endpoint.
- Query `/api/tags`.
- Show local models in UI.
- Recommend local model for compression/review.
- Keep all calls local-only.

## Phase 5 — Security hardening

Branch: `feature/security-hardening`

Goals:
- Harden Tauri CSP and capabilities.
- Review command allowlist.
- Add denylist for sensitive paths.
- Add tests for blocked actions.
- Add user-facing warnings before paid/cloud AI.

## Phase 6 — Packaging MVP

Branch: `feature/desktop-packaging`

Goals:
- Generate Tauri app icons.
- Build desktop app locally.
- Add release checklist.
- Add backup/restore of local RepoDesk state.

## What not to do yet

Avoid:
- Arbitrary shell execution from UI.
- Automatic code patching without Git visibility and guard checks.
- Sending full repository context to paid AI.
- Adding many new panels before the workflow screen is clear.
