# Code Editor Visual Ownership Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Retire the `code-editor-polish.css` naming/debt pattern without changing Code editor rendering, geometry, loading, or cascade semantics.

**Architecture:** Keep editor CSS globally owned by `App.css` in the existing `legacy` cascade layer, but move the stylesheet to the canonical app-style path `apps/desktop/src/app/styles/code-editor.css`. Add an architecture ownership contract first, observe RED on the missing canonical file/import, then make a byte-preserving stylesheet move, replace the import in place, and correct only stale comments.

**Tech Stack:** React/TypeScript, CSS cascade layers, Node.js architecture-ratchet tests, GitHub Actions, Playwright, Tauri/WebDriverIO.

## Global Constraints

- No TSX behavior, editor event logic, geometry constants, routing, backend/Rust, or domain changes.
- Preserve the stylesheet's effective declarations and `layer(legacy)` loading position.
- `apps/desktop/src/features/code/code-editor-polish.css` must not exist after migration.
- `apps/desktop/src/app/styles/code-editor.css` must exist after migration.
- `App.css` must import `./styles/code-editor.css` with `layer(legacy)` at the former legacy import position.
- `code-workspace.css` may change comments only; declarations must remain unchanged.
- Keep the zero-`*-polish.css` ratchet and the freeze on new `-vN.css` generations.
- Keep the 28 KiB source architecture hard limit and all existing semantic design-system contracts.

---

### Task 1: Encode and implement canonical Code editor visual ownership

**Files:**
- Modify: `scripts/design-system-ratchet.test.mjs`
- Modify: `scripts/check-source-architecture.mjs`
- Create: `apps/desktop/src/app/styles/code-editor.css`
- Modify: `apps/desktop/src/app/App.css`
- Modify: `apps/desktop/src/features/code/code-workspace.css`
- Delete: `apps/desktop/src/features/code/code-editor-polish.css` (already removed on the branch; restore behavior at the canonical path rather than reintroducing this file)

**Interfaces:**
- Produces: `evaluateCodeEditorVisualOwnershipContract(): string[]`
- Consumes: tracked repository files plus `App.css` and `code-workspace.css` source text.

- [ ] **Step 1: Write the failing ownership test**

Extend `scripts/design-system-ratchet.test.mjs` with a test that requires `architecture.evaluateCodeEditorVisualOwnershipContract` to exist and then requires the live repository state to return no failures. The contract must also reject an old `code-editor-polish.css` reference when supplied through its testable inputs.

- [ ] **Step 2: Run Architecture Ratchet and verify RED**

Use the PR workflow on the exact test-only head. Expected: `Architecture Ratchet` fails specifically because `evaluateCodeEditorVisualOwnershipContract` is undefined / the canonical ownership contract is not implemented. Existing unrelated architecture tests must remain green.

- [ ] **Step 3: Add the minimal ownership evaluator**

Implement `evaluateCodeEditorVisualOwnershipContract()` in `scripts/check-source-architecture.mjs` so it verifies:

```text
apps/desktop/src/app/styles/code-editor.css exists
App.css contains @import "./styles/code-editor.css" layer(legacy);
App.css does not reference code-editor-polish.css
code-workspace.css does not reference code-editor-polish.css
```

Wire the evaluator into `runArchitectureRatchet()`.

- [ ] **Step 4: Preserve editor CSS at the canonical path**

Create `apps/desktop/src/app/styles/code-editor.css` from the exact contents of `apps/desktop/src/features/code/code-editor-polish.css` as present on base `main` (`249afe130236e87eb669a30080fe72168fda836c`). Do not alter declarations.

- [ ] **Step 5: Replace only the ownership references**

In `apps/desktop/src/app/App.css`, replace:

```css
@import "../features/code/code-editor-polish.css" layer(legacy);
```

with:

```css
@import "./styles/code-editor.css" layer(legacy);
```

at the same source position.

In `apps/desktop/src/features/code/code-workspace.css`, update only the stale comment that names `code-editor-polish.css`; leave declarations byte-equivalent otherwise.

- [ ] **Step 6: Run GREEN architecture verification**

Expected: Architecture Ratchet passes and the zero-polish contract also remains green.

- [ ] **Step 7: Review the final diff for behavioral equivalence**

Verify the PR shows the editor stylesheet as rename-equivalent content, App.css as a one-line import-path replacement, code-workspace.css as comment-only, plus architecture/spec/plan changes. No TSX/runtime source changes are allowed.

- [ ] **Step 8: Run exact-head merge gates**

Require all on the exact final head:

```text
Architecture Ratchet: success
CI: success
E2E (native): success
```

The CI result must include frontend build, fmt, clippy, Rust tests, secret scans, Playwright mock-IPC, coverage, cargo-deny, and gitleaks.

- [ ] **Step 9: Squash merge PR #200**

Only after exact-head verification and review-thread check, mark the PR ready and squash-merge with the verified head SHA.
