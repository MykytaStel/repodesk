# Projects Own Project Setup — Implementation Plan

> **Execution contract:** test-first, small commits, verification before merge.

**Goal:** move repository project registration/activation from global Settings into Projects → Registry without changing backend command semantics.

**Architecture:** extract project setup state and mutations into a project-domain hook, render the setup UI in `ProjectsTab`, remove setup ownership from `SettingsTab`/`useSettings`, and lock the boundary with an architecture regression test.

**Stack:** React 18, TypeScript, TanStack Query, Tauri invoke wrappers, Node test runner, GitHub Actions.

---

## Task 1 — RED: encode the ownership boundary

**Files**
- Modify: `scripts/check-source-architecture.test.mjs`

**Steps**
1. Add a test that reads Settings and Projects source files.
2. Assert Settings does not own/connect projects.
3. Assert project setup command ownership exists in `features/projects`.
4. Assert Projects no longer routes project setup/configuration to Settings.
5. Open the PR and verify Architecture Ratchet fails for the expected ownership assertions.

## Task 2 — GREEN: create project-domain setup hook

**Files**
- Create: `apps/desktop/src/features/projects/useProjectSetup.ts`

**Steps**
1. Move setup form state and notices into the project domain.
2. Preserve validation and `project_add` → `project_use` sequencing.
3. Preserve “already exists → activate” semantics.
4. Invalidate workspace, active project, workflow, registry and memory caches on success.
5. Add directory picking and basename defaulting here rather than in Settings.

## Task 3 — GREEN: make Projects Registry own setup UI

**Files**
- Modify: `apps/desktop/src/features/projects/ProjectsTab.tsx`
- Modify if needed: `apps/desktop/src/features/projects/projects-route.css`

**Steps**
1. Add local setup-panel visibility state.
2. Replace Settings redirect with local Add project action.
3. Render project name/path/type/language inputs and Browse action.
4. Wire Add and activate to the project hook.
5. Refresh registry after successful activation.
6. Remove misleading per-project Configure → Settings action until a true edit path exists.
7. Update empty-state copy.

## Task 4 — GREEN: remove setup ownership from Settings

**Files**
- Modify: `apps/desktop/src/features/settings/SettingsTab.tsx`
- Modify: `apps/desktop/src/features/settings/useSettings.ts`

**Steps**
1. Remove project picker imports and `showConnect` state from Settings.
2. Remove setup hook bindings and Connect project panel.
3. Remove project registration mutation/state from `useSettings`.
4. Remove dead setup-task state/mutation that is no longer consumed.
5. Keep provider settings and current project-memory behavior unchanged for this slice.

## Task 5 — Verify and refactor

**Verification**
1. Architecture Ratchet passes.
2. TypeScript/Vite build gates pass in CI.
3. Native E2E passes.
4. Review PR patch for accidental Settings/Projects cross-ownership.
5. Confirm `main` has not moved incompatibly; update branch if needed.
6. Squash-merge the PR into `main` only with green required workflows.

## Follow-up cut

Move `ProjectAiImportPanel` and Project Memory/Guidelines from Settings into Projects → Knowledge. That follow-up should remove the remaining project-scoped configuration from global Settings rather than expanding this slice beyond one ownership seam.
