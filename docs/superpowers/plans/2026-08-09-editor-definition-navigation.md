# Editor Definition Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add VS Code-style modifier-hover definition previews and a transient, precise destination reveal to RepoDesk's CodeMirror editor.

**Architecture:** `useLiveRustLanguage` remains the owner of rust-analyzer requests and exposes a cancellable position preview contract. `SemanticCodeEditor` owns modifier and pointer interpretation plus CodeMirror state decorations. The existing workspace-open hand-off carries the complete target range so navigation across tabs can reveal the exact symbol without creating a text selection.

**Tech Stack:** React 18, TypeScript, CodeMirror 6, TanStack Query, Tauri IPC, rust-analyzer LSP, Playwright.

## Global Constraints

- Implement on the currently checked-out branch as explicitly requested by the user.
- Do not commit or push; `AGENTS.md` requires explicit human authorization.
- Use existing CSS theme tokens and preserve the single right-side CodeMirror scrollbar.
- Command is the navigation modifier on macOS; Control is the modifier on Windows and Linux.
- The reveal lasts approximately 1.5 seconds and is not a native text selection.

---

### Task 1: Range-aware workspace navigation

**Files:**
- Modify: `apps/desktop/src/shared/api/codeWorkspace.ts`
- Modify: `apps/desktop/src/features/code/useLiveRustLanguage.tsx`
- Test: `apps/desktop/e2e/editor-ui.spec.ts`

**Interfaces:**
- Produces: `CodeWorkspaceLocation` with optional `endLine` and `endColumn` fields.
- Produces: `requestCodeWorkspaceOpen(path, { line, column, endLine, endColumn })` with positive-integer normalization.
- Consumes: `LanguageLocation.end_line` and `LanguageLocation.end_column` from rust-analyzer.

- [ ] **Step 1: Write the failing destination-range test**

Add a Playwright test that writes a one-shot location containing start and end coordinates into session storage, dispatches `repodesk:open-code`, and expects `.cm-navigation-target` to cover text while `.cm-navigation-target-line` marks exactly one line and CodeMirror's selection remains collapsed.

- [ ] **Step 2: Run the focused test and verify RED**

Run: `pnpm --dir apps/desktop exec playwright test e2e/editor-ui.spec.ts --grep "reveals the exact navigation target"`

Expected: FAIL because the location range is discarded and no navigation decoration exists.

- [ ] **Step 3: Extend the hand-off and all language-location callers**

Normalize optional end coordinates in `requestCodeWorkspaceOpen` and `consumeCodeWorkspaceLocation`. Pass `end_line` and `end_column` for single definitions, location result buttons, and document-symbol navigation.

- [ ] **Step 4: Add the minimal CodeMirror reveal state**

Add state effects and a state field that produce one mark decoration and one line decoration, map safely through document changes, and clear on explicit effects. Apply the range when consuming a pending location, center it, retain a collapsed caret at the start, and schedule cleanup after 1,500 ms.

- [ ] **Step 5: Run the focused test and verify GREEN**

Run: `pnpm --dir apps/desktop exec playwright test e2e/editor-ui.spec.ts --grep "reveals the exact navigation target"`

Expected: PASS.

### Task 2: Modifier-hover definition preview

**Files:**
- Modify: `apps/desktop/src/features/code/useLiveRustLanguage.tsx`
- Modify: `apps/desktop/src/features/code/SemanticCodeEditor.tsx`
- Modify: `apps/desktop/src/features/code/semantic-code-editor.css`
- Modify: `apps/desktop/e2e/mock-ipc.ts`
- Modify: `apps/desktop/e2e/editor-ui.spec.ts`

**Interfaces:**
- Produces: `actions.previewAt(position): Promise<{ hover: LanguageHover | null; definitions: LanguageLocation[] } | null>`.
- Produces: `actions.clearPreview()` to invalidate outstanding preview requests and dismiss transient hover UI.
- Consumes: CodeMirror pointer coordinates and platform modifier key state.

- [ ] **Step 1: Write failing modifier-hover tests**

Add Rust editor fixtures and action-aware mock IPC responses. Test that modifier-hover requests the clicked source position, adds `.cm-definition-link`, displays an anchored `.cm-definition-preview`, does not move the caret, and clears when the modifier is released.

- [ ] **Step 2: Run focused tests and verify RED**

Run: `pnpm --dir apps/desktop exec playwright test e2e/editor-ui.spec.ts --grep "modifier-hover"`

Expected: FAIL because the editor has no modifier-hover decorations or anchored preview.

- [ ] **Step 3: Add cancellable position preview behavior**

In `useLiveRustLanguage`, request hover and definitions for the same position, use a monotonically increasing request identity to discard stale results, avoid the global panel for transient previews, and expose a clear action.

- [ ] **Step 4: Add CodeMirror modifier tracking and decorations**

Track Command/Control keydown and keyup plus pointer movement. Debounce preview requests briefly, resolve the hover range to document offsets, decorate only targets with at least one definition, and render a non-focus-stealing preview card positioned within the editor host. Clear it on pointer exit, modifier release, Escape, document edits, or navigation.

- [ ] **Step 5: Style the source link and preview card**

Use theme tokens for the underline, pointer cursor, surface, border, muted text, monospace content, and shadow. Keep decorations clipped to CodeMirror content and respect `prefers-reduced-motion`.

- [ ] **Step 6: Run focused tests and verify GREEN**

Run: `pnpm --dir apps/desktop exec playwright test e2e/editor-ui.spec.ts --grep "modifier-hover"`

Expected: PASS.

### Task 3: Interaction cleanup and regression gate

**Files:**
- Modify: `apps/desktop/src/features/code/SemanticCodeEditor.tsx`
- Modify: `apps/desktop/src/features/code/semantic-code-editor.css`
- Test: `apps/desktop/e2e/editor-ui.spec.ts`

**Interfaces:**
- Consumes: Task 1 reveal effects and Task 2 transient preview actions.
- Produces: cleanup behavior for edits, selection changes, timeout, Escape, and new navigation.

- [ ] **Step 1: Add failing cleanup and normal-click assertions**

Verify a normal click makes no language request, an edit clears destination decorations immediately, and the timeout clears them without changing the caret.

- [ ] **Step 2: Run cleanup tests and verify RED**

Run: `pnpm --dir apps/desktop exec playwright test e2e/editor-ui.spec.ts --grep "navigation target|normal click"`

Expected: at least the cleanup assertion FAILS before cleanup wiring is complete.

- [ ] **Step 3: Implement cleanup paths**

Clear preview and reveal decorations on the specified user actions, cancel timers during path changes and unmount, and ensure asynchronous preview completion cannot restore cleared state.

- [ ] **Step 4: Run focused editor tests**

Run: `pnpm --dir apps/desktop exec playwright test e2e/editor-ui.spec.ts`

Expected: all editor UI tests PASS.

- [ ] **Step 5: Run frontend verification**

Run: `pnpm --dir apps/desktop run build`

Expected: TypeScript and Vite build PASS with no errors.

- [ ] **Step 6: Run repository diff checks**

Run: `git diff --check`

Expected: PASS with no whitespace errors; review `git status --short` to ensure only scoped files changed.
