# RepoDesk Trust Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align RepoDesk's visible identity, modal behavior, project errors, confirmations, and initial bundle with its local-first engineering-workspace contract.

**Architecture:** Introduce one accessible modal primitive plus a promise-based decision hook, propagate project-query failures instead of converting them to empty data, and defer Xterm behind a first-activation boundary. Protect the behavior with Playwright and a production entry-budget gate.

**Tech Stack:** React 18, TypeScript, TanStack Query, Playwright, Vite 5, Tauri 2.

## Global Constraints

- Preserve the five primary surfaces: `Work`, `Code`, `Changes`, `Runs`, `Projects`.
- Keep all destructive actions explicit and user-confirmed.
- Preserve PTY sessions after the first Terminal activation.
- Do not weaken the existing 500 kB per-chunk budget.
- Initial JavaScript must remain below 110 kB gzip and must not preload Xterm or CodeMirror.

---

### Task 1: Trust regression coverage

**Files:**
- Create: `apps/desktop/e2e/trust-polish.spec.ts`
- Modify: `apps/desktop/e2e/mock-ipc.ts`
- Create: `apps/desktop/scripts/check-entry-budget.mjs`
- Modify: `apps/desktop/package.json`

**Interfaces:**
- Produces: mock fixture `{ __mock_error: string }` rejection behavior.
- Produces: `pnpm build` entry-budget gate.

- [ ] Write Playwright tests for product identity, accessible About behavior, honest project errors, and RepoDesk-owned Orchestrate confirmation.
- [ ] Add the production entry-budget check and wire it after Vite build.
- [ ] Run focused tests/build and confirm they fail for the expected old behavior.

### Task 2: Shared accessible dialog and product identity

**Files:**
- Create: `apps/desktop/src/shared/ui/Dialog.tsx`
- Create: `apps/desktop/src/shared/ui/useDecisionDialog.tsx`
- Modify: `apps/desktop/src/shared/ui/AboutModal.tsx`
- Modify: `apps/desktop/src/shared/ui/ArtifactViewerModal.tsx`
- Modify: `apps/desktop/src/features/code/IdeDecisionDialog.tsx`
- Modify: `apps/desktop/src/features/dashboard/DashboardTab.tsx`
- Modify: `docs/ROADMAP.md`

**Interfaces:**
- Produces: `Dialog` with Escape, focus containment, and focus restoration.
- Produces: `useDecisionDialog()` returning `{ confirm, dialog }`.

- [ ] Implement the minimal shared dialog behavior required by the failing tests.
- [ ] Migrate About and Artifact Viewer to the shared primitive.
- [ ] Re-export Code's decision hook through the shared implementation.
- [ ] Align visible legacy copy with the engineering-workspace identity.
- [ ] Run the focused identity/dialog tests and confirm green.

### Task 3: Honest project loading and native product decisions

**Files:**
- Modify: `apps/desktop/src/app/App.tsx`
- Modify: `apps/desktop/src/app/ProjectSwitcher.tsx`
- Modify: `apps/desktop/src/features/orchestrate/OrchestrateTab.tsx`

**Interfaces:**
- Consumes: `useDecisionDialog()` from Task 2.
- Consumes: `__mock_error` fixture behavior from Task 1.

- [ ] Allow the shared project-list query to retain its error state.
- [ ] Render a bounded project-registry error with Retry in the switcher.
- [ ] Replace Orchestrate `alert()` and `confirm()` calls with RepoDesk decisions.
- [ ] Run the focused project/Orchestrate tests and confirm green.

### Task 4: Lazy Terminal and entry budget

**Files:**
- Modify: `apps/desktop/src/app/WorkbenchBottomPanel.tsx`

**Interfaces:**
- Produces: first-activation lazy import while preserving the mounted terminal thereafter.

- [ ] Replace the eager Xterm import with `React.lazy`.
- [ ] Track first Terminal activation across button and workbench events.
- [ ] Keep the lazy component mounted after activation and pass the live `active` flag.
- [ ] Run `pnpm --dir apps/desktop build` and confirm the entry-budget gate passes.

### Task 5: Full verification and delivery

**Files:**
- Review all changed files.

- [ ] Run focused Playwright tests.
- [ ] Run all desktop E2E tests.
- [ ] Run `./scripts/verify-all.sh`.
- [ ] Run `cargo clippy --all-targets --all-features -- -D warnings` and `git diff --check`.
- [ ] Review the final diff for trust-copy, focus, error, and confirmation regressions.
- [ ] Commit, push `feature/trust-polish`, and open a draft PR.

