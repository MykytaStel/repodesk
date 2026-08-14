# Projects Own Project Setup — Design

**Date:** 2026-08-15  
**Status:** Approved for implementation

## Problem

RepoDesk presents **Projects** as the durable boundary for repository rules, knowledge and reusable work setup, but project registration still lives in **Settings**. `ProjectsTab` redirects both “Add or configure project” and per-project “Configure” actions to Settings, while `useSettings` owns `project_add` / `project_use` mutations.

That contradicts the product information architecture: Settings is described as global provider/key/preferences configuration, while Projects is the project-scoped surface.

## Goal

Make **Projects → Registry** the single UI owner for connecting/activating repository projects.

After this slice:

- a user can add and activate a project directly from Projects;
- project setup state/mutations live under `features/projects`;
- Settings no longer exposes or owns repository registration;
- Projects no longer routes “Add” or “Configure” into Settings;
- the ownership boundary is guarded by an automated architecture test.

## Non-goals

This slice deliberately does **not**:

- redesign the backend `project_add` / `project_use` protocol;
- move Project Memory / Guidelines out of Settings yet;
- move `ProjectAiImportPanel` yet;
- add project editing/deletion backend commands that do not already exist;
- redesign the full Projects surface.

Memory/import ownership is the next project-scoped convergence cut.

## Architecture

### New project-domain hook

Create `apps/desktop/src/features/projects/useProjectSetup.ts`.

It owns:

- setup form state (`projectName`, `projectPath`, `projectType`, `mainLanguage`);
- directory selection and folder-name defaulting;
- `project_add` followed by `project_use`;
- success/error notice state;
- invalidation of workspace/project registry/workflow/memory queries after activation.

The hook uses the existing command contracts and preserves the current “already exists → activate it” behavior.

### Projects registry UI

`ProjectsTab` owns an inline, collapsible setup panel in the Registry view.

- “Add project” toggles the panel locally.
- Empty-state copy points to the local setup action, not Settings.
- Per-project `Configure` does not route to global Settings. Until a real edit command exists, the misleading action is removed rather than pretending Settings can edit project configuration.
- Existing “Open project” behavior remains unchanged.

### Settings cleanup

`SettingsTab` removes:

- project directory picker imports;
- setup form/setup notice/add-project mutation bindings;
- local `showConnect` state;
- the “Workspace / Connect a project” panel.

`useSettings` removes project registration and unrelated dead setup-task state. It remains responsible for global provider settings and, temporarily, project memory until the follow-up ownership slice.

## Error and consistency semantics

- Empty project name/path fails before backend mutation.
- `project_add` failures are surfaced unless the project already exists.
- `project_use` failure is distinct: registration may have succeeded but activation failed.
- Successful activation invalidates the canonical workspace snapshot and project registry cache so the newly active project is reflected without a reload.

## Test contract

Add a source-ownership regression test to `scripts/check-source-architecture.test.mjs`.

The test must fail while Settings owns project setup and pass only when:

1. `SettingsTab.tsx` no longer contains the project-connect UI or project-setup hook bindings;
2. `useSettings.ts` no longer calls `project_add` / `project_use`;
3. project setup command ownership exists under `features/projects`;
4. `ProjectsTab.tsx` no longer routes setup/configuration to `settings`.

This is intentionally an architecture test: the regression being prevented is ownership drift, not a single DOM rendering detail.

## Acceptance criteria

- Project registration/activation is usable from Projects → Registry.
- Settings has no project-connect panel.
- `features/settings/useSettings.ts` has no `project_add` or `project_use` command path.
- Project setup implementation resides in `features/projects`.
- No Projects setup/configuration button redirects to Settings.
- TypeScript/build gates pass.
- Architecture Ratchet, CI and native E2E are green before squash merge to `main`.
