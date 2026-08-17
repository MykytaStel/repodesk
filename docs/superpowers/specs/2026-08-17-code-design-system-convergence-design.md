# Code Design-System Convergence Design

Date: 2026-08-17
Status: approved Cut F slice
Roadmap: Cut F — Code shell/state migration

## Goal

Complete the fifth owning-surface migration in Cut F by converging Code on the shared semantic UI language without redesigning the editor or changing repository mutation, draft recovery, language-server, or CodeMirror behavior.

Code remains an editor-first engineering surface. The migration standardizes route authority, typed engineering state, errors/loading/empty states, and technical evidence while preserving specialized IDE geometry.

## Current constraints and debt

The current Code surface has several important characteristics that constrain the migration:

- `CodeTab.tsx` is above the 28 KiB source architecture limit and therefore may only stay flat or shrink;
- `SemanticCodeEditor.tsx` is close to the limit and owns specialized CodeMirror behavior that should not absorb more route semantics;
- the route shell still uses the historical `code-workspace-v0` class;
- Code has typed engineering state but `SemanticStrip` translates it with local ad-hoc classes instead of the shared semantic vocabulary;
- `CodeWorkspaceTree.tsx` maps typed file statuses through a local `statusTone()` helper;
- route-level empty/loading/error feedback still uses legacy `focus-empty` / `notice` presentation;
- repository-intelligence loading/error/evidence presentation is local despite being technical evidence UI;
- historical `code-editor-polish.css` exists, but it is not part of the active `SemanticCodeEditor` import boundary and belongs to the final Cut F historical CSS/dead-class cleanup unless an active consumer is proven during implementation.

## Architecture

Use one Code-local typed semantic adapter:

```text
Code / engineering typed state
  -> codeSemantic.ts
  -> SemanticState / SemanticTone
  -> shared primitives
  -> Code shell, explorer, semantic strip, technical inspector
```

Do not import semantic adapters from another feature such as Changes. Equivalent domain states should use equivalent tones, but Code owns its own presentation mapping so feature dependencies remain one-way through shared APIs and primitives.

### Typed adapter responsibilities

`codeSemantic.ts` owns exhaustive mappings for:

- `CodeWorkspaceFileStatus`;
- `ChangeFileScopeState`;
- `ChangeReviewState`;
- `ChangeVerificationState`, including the local `dirty-after-passed` condition;
- `SemanticOrigin`;
- `RepositoryEvidenceLevel`;
- workspace-index completeness (`complete` / `truncated`);
- editor save state (`saved` / `dirty` / `saving`) where a semantic badge is useful.

Unknown typed variants must not silently become `neutral`; exhaustive switches use an `assertNever` guard.

### Route ownership

`CodeTab.tsx` remains the orchestration owner for:

- workspace query lifecycle;
- file-open/tab lifecycle;
- draft persistence and recovery;
- save mutation;
- Code workspace mutations;
- navigation to Changes;
- RepoPilot review state;
- inspector visibility.

Pure tab/session transformations move into a focused helper module so the route drops below the 28 KiB source limit without moving business ownership into a UI component.

### Editor ownership

`SemanticCodeEditor.tsx` continues to own CodeMirror construction, syntax, diagnostics, gutters, navigation, live-language behavior, and editor-specific status geometry.

Extract the semantic engineering strip into `CodeSemanticStrip.tsx`. The strip consumes the typed `SemanticFileState`, maps it through `codeSemantic.ts`, and renders compact shared `StatusBadge` / `EvidenceState` presentation without wrapping the editor in dashboard panels.

The editor footer (cursor, language, encoding, line count, bytes/chars) remains specialized editor chrome rather than being converted to generic metrics.

### Explorer ownership

`CodeWorkspaceTree.tsx` keeps virtualization and tree interaction. Its dynamic inline row geometry is data-driven and remains a legitimate exception to the static-inline-style freeze.

Replace local `statusTone()` with `codeFileStatusSemantic(...)`. Compact file state should use the shared semantic status vocabulary while preserving single-letter Git labels where density matters.

### Repository intelligence

`RepositoryIntelligenceDrawer.tsx` remains a specialized technical drawer. Migrate:

- loading to shared `LoadingState`;
- query failure to shared `ErrorState`;
- graph evidence level to `StatusBadge` via `codeSemantic.ts`.

Do not rebuild every drawer section into card components. Existing dense technical sections remain appropriate inspector geometry.

## Route states

### No active project

Use a surface-scoped `EmptyState`. Code is unavailable because the route lacks project authority; this is not an error.

### Workspace loading

Use a surface-scoped `LoadingState` with the existing indexing language.

### Workspace authority failure

Use a surface-scoped `ErrorState` with the useful backend error detail.

### Indexed workspace

The canonical root is `.code-workspace`, not `.code-workspace-v0`.

The dense toolbar remains IDE chrome. It may use semantic `StatusBadge` state for index completeness and unsaved state, but must not be forced into a dashboard `PanelHeader` layout.

### Local mutation / draft failures

Local failures stay local and visible. Critical workspace errors use `ErrorState` in the editor workbench. Draft-backup/recovery warnings use attention semantic evidence/status rather than a critical route failure.

A reload-from-disk recovery action remains attached to the local workspace error when the conflict-safe save reports that the file changed outside RepoDesk.

### Document empty/loading states

No-open-file and diff loading/empty states use shared empty/loading vocabulary at inline scope while preserving editor-stage geometry.

## Actions

Do not introduce a generic route primary action. Code is a tool surface and most toolbar actions are secondary navigation/inspection commands.

The document context owns at most one primary mutation action: `Save`. Existing Edit/Diff view switching and route toolbar icon actions remain secondary controls.

`ActionBar` may be used only where it preserves the dense document toolbar. If the primitive's default geometry would distort the editor, preserve the specialized toolbar and enforce the one-primary invariant through Code E2E/architecture contracts instead of creating a competing layout abstraction.

## CSS contract

This slice must reduce visual-generation debt without creating new CSS:

- rename `.code-workspace-v0` to canonical `.code-workspace` in the existing stylesheet and TSX consumer;
- no new feature-local CSS files;
- existing feature CSS may only stay flat or shrink;
- no new raw TSX hex values;
- no new static inline layout styles;
- preserve data-driven editor/tree inline positioning exceptions;
- do not create `code-v1`, `code-v2`, `code-polish`, or equivalent generations.

`code-editor-polish.css` is intentionally reserved for the final Cut F historical CSS/dead-class cleanup unless an active import/consumer is proven in this slice. Renaming unused historical CSS is not semantic convergence.

## Source architecture contract

After migration:

- `CodeTab.tsx` must be at or below 28 KiB;
- one `codeSemantic.ts` adapter must exist;
- migrated Code semantic surfaces must consume the shared primitive boundary;
- migrated Code typed surfaces must consume the Code adapter;
- `CodeWorkspaceTree.tsx` must not define or call `statusTone()`;
- `CodeTab.tsx` must not use `code-workspace-vN` classes;
- no status substring inference may classify typed Code state.

The Code contract is added to the existing architecture ratchet and cannot weaken Changes, Work, Runs, Projects, or Work visual-debt contracts.

## Testing contract

### Playwright

Add `code-design-system.spec.ts` with representative state coverage:

- no active project -> surface empty state;
- workspace loading -> surface loading state;
- workspace query failure -> surface critical error state;
- normal workspace -> canonical `.code-workspace`, no `.code-workspace-v0`;
- truncated index -> attention semantic status;
- typed file status -> semantic tone in Explorer;
- active-file engineering state -> typed scope / verification presentation;
- local workspace error remains local and critical;
- Code keeps at most one document primary action (`Save`).

Existing editor, draft recovery, file operations, virtualization, theme, IDE health, and native E2E tests remain authoritative regression coverage.

### Architecture

Extend `scripts/check-source-architecture.mjs` and `scripts/design-system-ratchet.test.mjs` so Code convergence cannot regress.

## Non-goals

- no CodeMirror rewrite;
- no editor engine replacement;
- no language-server behavior changes;
- no repository search algorithm changes;
- no draft persistence/recovery behavior changes;
- no file mutation semantics changes;
- no backend/Rust changes;
- no complete repository-intelligence redesign;
- no final repository-wide historical CSS cleanup in this PR.

## Merge gate

Before squash merge, require the exact PR head to have:

- Architecture Ratchet green;
- full CI green, including frontend build, fmt, Clippy, Rust tests, Playwright, coverage, cargo-deny, gitleaks and strict secret scan;
- native Tauri/WebDriverIO E2E green;
- final diff review showing no editor/domain scope creep;
- `main` unchanged from or cleanly mergeable with the reviewed base.
