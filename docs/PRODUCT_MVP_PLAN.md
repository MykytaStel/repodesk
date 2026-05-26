# RepoDesk Product MVP Plan

RepoDesk should become a daily desktop cockpit for AI-assisted development.
The product must help the user understand the project state, build safe context, choose the right AI/runtime, run checks, and avoid losing work.

## Product promise

Open RepoDesk and understand:

- Which project is active.
- Which task is active.
- What changed in Git.
- What the next safe step is.
- Which AI/runtime is available.
- Whether context is safe and affordable.
- Whether checks pass.
- What RepoDesk did recently.

## Current product state

Already built or started:

- Rust core workspace.
- CLI project/task workflow.
- Context and smart-context artifacts.
- Token/budget guard.
- Prompt generation.
- Checks runner and summaries.
- Security/safety/judge concepts.
- Runtime/adapters registry.
- AI discovery scanning.
- Tauri desktop UI.
- UI workflow/actions/debug panels.
- Git workspace awareness.
- Basic SQLite/settings direction.

## MVP definition

RepoDesk is MVP-ready for daily use when these are true:

1. Desktop opens reliably.
2. Project can be added/selected from UI.
3. Task can be created/selected from UI.
4. Git status is visible.
5. Workflow shows one clear next safe action.
6. Smart context can be built from UI.
7. Prompt can be viewed/copied from UI.
8. Checks can be run from UI.
9. AI discovery shows found/missing tools.
10. Debug tab shows what happened.
11. Health report/debug bundle can be generated.
12. Secret scan exists before commit.

## Near-term feature order

### 1. Stability and optimization

Branch: `feature/stability-optimization`

Focus:
- verify scripts
- health report
- debug bundle
- basic secret scan
- roadmap/security docs

### 2. Product workflow hardening

Branch: `feature/product-workflow-hardening`

Focus:
- one primary action
- better empty states
- ready-to-commit checklist
- before/after action visibility
- fewer raw/debug-first panels

### 3. SQLite state store

Branch: `feature/sqlite-state-store`

Focus:
- settings
- provider preferences
- sessions
- events
- action history
- task metadata

### 4. Ollama health runtime

Branch: `feature/ollama-health-runtime`

Focus:
- `/api/tags`
- local model list
- model health
- local routing recommendations

### 5. Security hardening

Branch: `feature/security-hardening`

Focus:
- Tauri command allowlist
- deny sensitive paths
- prevent unsafe logs
- paid AI warnings
- blocked action tests

### 6. Desktop packaging

Branch: `feature/desktop-packaging`

Focus:
- icon set
- local app build
- release checklist
- backup/restore of local state

## What to avoid now

- Do not add arbitrary shell execution from UI.
- Do not auto-patch code without Git visibility.
- Do not send full repo context to paid AI.
- Do not add many new panels before workflow is clear.
- Do not store API keys in plain local config.
