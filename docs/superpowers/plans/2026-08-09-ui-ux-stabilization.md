# RepoDesk UI/UX Stabilization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the current editor, layout, readability, state, responsive, and repository-hygiene defects while preserving RepoDesk's existing product workflow.

**Architecture:** Keep the textarea as the editor's sole scroll owner and extract its geometry into one testable contract shared by the gutter and cursor marker. Normalize cross-app behavior through existing foundation tokens and shared control selectors, then keep feature layout ownership in focused feature stylesheets. Exercise the rendered application through the existing mocked-Tauri Playwright harness.

**Tech Stack:** React 18, TypeScript, CSS custom properties, Vite, Playwright, pnpm, Tauri 2.

## Global Constraints

- Preserve the approved balanced IDE density and existing information architecture.
- Important interface text and actionable labels must be at least 12 px.
- Use existing theme tokens; do not hardcode feature colors.
- Do not introduce Monaco, CodeMirror, or a new dependency.
- Preserve unrelated work and do not commit or push without explicit approval.
- The source editor is the sole scroll owner; the gutter never owns a scrollbar.

---

### Task 1: Editor and gutter regression contract

**Files:**
- Create: `apps/desktop/e2e/editor-ui.spec.ts`
- Modify: `apps/desktop/e2e/fixtures.ts`
- Modify: `apps/desktop/src/features/code/LightweightCodeEditor.tsx`
- Modify: `apps/desktop/src/features/code/code-editor-polish.css`
- Modify: `apps/desktop/src/features/code/code-workspace.css`

**Interfaces:**
- Consumes: existing mocked Tauri IPC and Code workspace navigation.
- Produces: one `.code-editor-input` scroll owner, one `.code-editor-gutter-shell`, one active marker that does not duplicate a track number, and shared editor geometry custom properties.

- [ ] Add a Playwright fixture containing a long source file with long horizontal lines, trailing newline coverage, and at least 200 rows.
- [ ] Add a failing test that opens the file and asserts the gutter has `overflow: hidden`, no scrollbars, and the textarea is scrollable.
- [ ] Add a failing test that selects line 7 and asserts exactly one visible `7` occupies the active-number position.
- [ ] Add a failing test that scrolls to the middle and end and compares the active marker and source caret row within one CSS pixel.
- [ ] Run `pnpm --dir apps/desktop exec playwright test e2e/editor-ui.spec.ts` and confirm each new assertion fails for the reported behavior.
- [ ] Replace duplicate active-number rendering with a single representation while keeping screen-reader behavior on the textarea.
- [ ] Define top padding, line height, gutter width, and gutter inset once on `.code-editor-body`; consume those values in the textarea, track, and active marker.
- [ ] Ensure only textarea scroll events update `--editor-scroll-top` and the gutter shell cannot scroll independently.
- [ ] Re-run the focused editor test and confirm it passes.

### Task 2: Shared readable-density and interaction states

**Files:**
- Modify: `apps/desktop/src/app/styles/foundation.css`
- Modify: `apps/desktop/src/app/styles/components.css`
- Modify: `apps/desktop/src/app/styles/layout.css`
- Modify: `apps/desktop/src/app/styles/responsive.css`
- Modify: `apps/desktop/src/app/App.css`
- Test: `apps/desktop/e2e/ui-foundation.spec.ts`

**Interfaces:**
- Consumes: existing theme variables and shared button/input classes.
- Produces: canonical type, control, focus, disabled, panel-width, and responsive behavior used by feature surfaces.

- [ ] Add failing rendered-style assertions for primary labels below 12 px, invisible keyboard focus, weak disabled contrast, and clipped controls at narrow width.
- [ ] Run the focused test and confirm failures identify existing style ownership conflicts.
- [ ] Add or correct foundation tokens for readable text, balanced control heights, disabled opacity, focus ring, and bounded content widths.
- [ ] Consolidate duplicate shared selectors so later imports consume tokens instead of changing component geometry.
- [ ] Remove obsolete conflicting overrides only where covered by the new assertions.
- [ ] Verify the focused suite in dark, light, and midnight themes at 1280×800 and 760×720.

### Task 3: Knowledge and Work surface stabilization

**Files:**
- Modify: `apps/desktop/src/features/knowledge/knowledge.css`
- Modify: `apps/desktop/src/features/knowledge/knowledge-v2.css`
- Modify: `apps/desktop/src/features/knowledge/KnowledgeTab.tsx`
- Modify: `apps/desktop/src/app/styles/focus-shell.css`
- Modify: `apps/desktop/src/app/styles/focus-density.css`
- Modify: `apps/desktop/src/features/work/WorkSurface.tsx`
- Test: `apps/desktop/e2e/work-golden-path.spec.ts`
- Test: `apps/desktop/e2e/knowledge-ui.spec.ts`

**Interfaces:**
- Consumes: shared tokens and controls from Task 2.
- Produces: bounded empty/form states, readable filter tabs, compact Work toolbar, and predictable responsive stacking.

- [ ] Add failing Knowledge assertions for non-overlapping counts/labels, one clear active state, bounded empty content, form maximum width, readable labels, and disabled submit contrast.
- [ ] Add failing Work assertions that the toolbar and empty scope state fit the viewport without oversized blank bands or horizontal overflow.
- [ ] Run the two focused suites and confirm the new assertions fail.
- [ ] Merge conflicting Knowledge base/v2 geometry into one effective owner per component.
- [ ] Bound the proposal form, use balanced category/title columns, and stack them below the supported narrow breakpoint.
- [ ] Normalize Knowledge tabs so their numeral and label cannot double or overlap in active state.
- [ ] Tighten Work toolbar and empty scope composition without changing navigation or workflow behavior.
- [ ] Re-run the focused suites in dark and light themes.

### Task 4: Remaining primary-tab audit and repairs

**Files:**
- Modify as findings require: `apps/desktop/src/app/styles/*.css`
- Modify as findings require: `apps/desktop/src/features/*/*.css`
- Modify as findings require: `apps/desktop/src/shared/ui/*.tsx`
- Modify as findings require: `apps/desktop/src/features/*/*.tsx`
- Test: `apps/desktop/e2e/ui-audit.spec.ts`
- Test: `apps/desktop/e2e/daily-loop.spec.ts`

**Interfaces:**
- Consumes: the existing `APP_TABS` catalog and mocked IPC fixtures.
- Produces: a reproducible route-by-route audit for clipping, overflow, state visibility, focus, themes, narrow layout, and console errors.

- [ ] Add a data-driven Playwright audit that opens every primary and advanced tab at normal width in dark and light themes.
- [ ] Fail on horizontal document overflow, clipped primary controls, inaccessible focus, page-level console errors, or missing visible page content.
- [ ] Repeat the audit at 760 px width and assert controls remain inside the viewport or reflow intentionally.
- [ ] Run the audit and record each failure by route and selector.
- [ ] Fix shared causes first, one behavior at a time, rerunning the affected route after every change.
- [ ] Fix genuinely local defects in their owning feature stylesheet or component without unrelated refactors.
- [ ] Run `pnpm --dir apps/desktop exec playwright test e2e/ui-audit.spec.ts e2e/daily-loop.spec.ts` until clean.

### Task 5: Repository hygiene

**Files:**
- Modify: `.gitignore`
- Test: shell-level ignore and status checks.

**Interfaces:**
- Consumes: actual artifacts produced by local build, Playwright, Tauri, debugging, and brainstorming workflows.
- Produces: precise ignore rules that exclude generated/local data without hiding project source or fixtures.

- [ ] Capture `git status --short --untracked-files=all` and classify every untracked generated path.
- [ ] Verify each candidate with `git ls-files` and `git check-ignore -v` before adding a pattern.
- [ ] Add `.superpowers/` and any other confirmed generated path absent from the current rules.
- [ ] Confirm representative required files remain trackable: `Cargo.lock`, migrations, Tauri icons, `.env.example`, Playwright specs, and documentation.
- [ ] Run normal frontend build and focused E2E tests, then confirm no new generated artifact appears in `git status`.

### Task 6: Full verification and runtime visual QA

**Files:**
- Modify only defects found by verification.
- Update: `docs/superpowers/specs/2026-08-09-ui-ux-stabilization-design.md` only if implementation reveals a necessary clarified constraint.

**Interfaces:**
- Consumes: completed Tasks 1–5.
- Produces: release-quality evidence that the reported defects and audit findings are resolved.

- [ ] Run `pnpm --dir apps/desktop run build`.
- [ ] Run the complete Playwright suite with `pnpm --dir apps/desktop exec playwright test`.
- [ ] Run `./scripts/verify-fast.sh` with stateful paths isolated as required by `AGENTS.md`.
- [ ] Launch the desktop-compatible frontend and visually inspect every primary tab at 1280×800 and 760×720 in dark and light themes.
- [ ] Inspect the editor at the beginning, middle, and end of the long fixture and confirm the source owns the only scrollbar, numbers stay aligned, and the active number is crisp.
- [ ] Inspect browser console output and correct any errors introduced or exposed by the changes.
- [ ] Review the final diff for accidental broad styling, generated files, secrets, and unrelated edits.
- [ ] Report changed files, verified commands, remaining pre-existing limitations, and whether the worktree contains only intentional changes.
