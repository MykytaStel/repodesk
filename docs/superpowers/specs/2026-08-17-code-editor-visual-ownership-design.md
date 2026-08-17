# Code Editor Visual Ownership Design

## Context

Cut F converged Changes, Work, Runs, Projects, and Code onto the shared semantic design system. During follow-up visual-debt cleanup, `apps/desktop/src/features/code/code-editor-polish.css` was initially treated as dead because repository code search returned no references. A direct inspection of `apps/desktop/src/app/App.css` disproved that assumption: the file is globally imported in the `legacy` cascade layer, and `LightweightCodeEditor.tsx` depends on selectors that are owned there, including the clipped line-number gutter, active-line marker, hidden native scrollbar, and custom scrollbar track/thumb.

Deleting the stylesheet without replacing its ownership would therefore be a runtime/editor regression. The cleanup must remove the legacy *name/ownership pattern* while preserving the exact editor CSS behavior.

## Decision

Move the existing stylesheet byte-for-byte from:

`apps/desktop/src/features/code/code-editor-polish.css`

into the canonical application style layer:

`apps/desktop/src/app/styles/code-editor.css`

and update `apps/desktop/src/app/App.css` to import the canonical path at the exact same position and with the exact same `layer(legacy)` assignment.

This is intentionally a visual-ownership migration, not a redesign.

## Why this owner

`App.css` already owns the ordered cascade for application-level style layers. The current editor stylesheet is loaded there globally rather than by `CodeTab` or `LightweightCodeEditor`. Moving it under `app/styles/` preserves the existing loading and cascade semantics while removing the route-local `*-polish.css` generation.

Alternative ownership in `features/code/code-workspace.css` was rejected for this PR because it would enlarge frozen feature CSS and merge two distinct concerns. Importing a new feature-local `code-editor.css` from the editor component was also rejected because it changes loading/cascade ownership and is unnecessary for the cleanup goal.

## Invariants

1. `apps/desktop/src/features/code/code-editor-polish.css` must not exist after the migration.
2. `apps/desktop/src/app/styles/code-editor.css` must exist and contain the same editor rules as the legacy stylesheet, except for comments that explicitly name the old path if such comments are corrected.
3. `apps/desktop/src/app/App.css` must import `./styles/code-editor.css` at the same position where the old stylesheet was imported and must keep `layer(legacy)`.
4. No TSX behavior, editor event logic, geometry constants, routing, backend/Rust, or domain contracts change.
5. `apps/desktop/src/features/code/code-workspace.css` may only receive a comment correction removing the obsolete claim that geometry lives in `code-editor-polish.css`; style declarations stay unchanged.
6. The generic zero-`*-polish.css` architecture contract remains enforced across `apps/desktop/src`.
7. Existing tracked `-vN.css` files remain a frozen baseline. New versioned generations remain forbidden, but canonicalizing existing live versioned files is out of scope.

## TDD / regression contract

The architecture ratchet must encode the desired ownership before the canonical move is implemented:

- fail when any tracked `*-polish.css` remains under `apps/desktop/src`;
- require `apps/desktop/src/app/styles/code-editor.css` to exist;
- require `App.css` to import the canonical file with `layer(legacy)`;
- reject any remaining `code-editor-polish.css` import/reference in `App.css` and the Code workspace stylesheet;
- preserve all existing semantic design-system contracts and the 28 KiB source budget.

The RED state should fail because the canonical file/import do not yet exist. GREEN then consists only of the CSS move, import replacement, and stale comment correction.

## Verification

On the exact final PR head:

- Architecture Ratchet must pass.
- Full CI must pass, including frontend build and Playwright smoke.
- Native Tauri/WebDriverIO E2E must pass.
- Final diff must show a rename-equivalent stylesheet move rather than deletion of editor behavior.
- No unrelated runtime source changes are allowed.

## Merge

Keep PR #200 as the isolated work contour. After exact-head verification and final diff review, mark it ready and squash-merge into `main` using the verified head SHA.
