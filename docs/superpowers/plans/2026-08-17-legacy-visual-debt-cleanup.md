# Legacy Visual Debt Cleanup Plan

## Goal

Finish the Cut F polish-layer cleanup after Changes, Work, Runs, Projects, and Code convergence by retiring the remaining tracked `*-polish.css` naming layer while preserving live editor behavior and enforcing a zero-polish baseline across desktop source.

## Evidence-driven scope

- Initial repository code search returned no references to `apps/desktop/src/features/code/code-editor-polish.css`, but direct inspection of `apps/desktop/src/app/App.css` later proved that result was a false negative: the stylesheet is globally imported in `layer(legacy)` and owns live `LightweightCodeEditor` gutter/scrollbar/geometry rules.
- The stylesheet must therefore be **moved, not dropped**. Its declarations are preserved byte-for-byte at `apps/desktop/src/app/styles/code-editor.css`; `App.css` keeps the same import position and `layer(legacy)` assignment.
- Existing tracked `-vN.css` stylesheets are **not** removed or globally failed in this PR. The first generic RED attempt proved multiple versioned stylesheets remain and may still be live (`ai-strategy-v1.css`, `command-palette-v2.css`, `knowledge-v2.css`, and others).
- The existing changed-file freeze already rejects **new** `-vN` visual generations and new `*-polish.css` stylesheets, so current versioned files remain a frozen baseline to canonicalize in separate, evidence-backed slices.
- Preserve feature CSS growth limits, Work canonical hierarchy ownership, and all Changes/Work/Runs/Projects/Code semantic contracts.
- No TSX behavior, editor event logic, backend/Rust, domain, or routing changes.

## TDD sequence

1. RED: require a generic legacy polish cleanup evaluator in the design-system ratchet. Confirm failure because the contract does not exist.
2. Implement the generic scanner and observe the broader `-vN` experiment. Use that failure as evidence to reject a blanket zero-versioned rule rather than deleting potentially live styles.
3. Refine the invariant to tracked `*-polish.css` only. Confirm the ratchet fails on exactly `apps/desktop/src/features/code/code-editor-polish.css`.
4. Direct import inspection disproves the initial dead-file assumption. Add a second RED ownership contract requiring canonical Code editor CSS, the canonical `App.css` import, and no stale polish references.
5. GREEN: move the exact stylesheet content to `apps/desktop/src/app/styles/code-editor.css`, replace the import path in place, and update only the stale workspace comment while keeping feature CSS at or below its frozen byte baseline.
6. Verify Architecture Ratchet, full CI, and native E2E on the exact final head.

## Merge gate

On the exact final head require:

- Architecture Ratchet green.
- Full CI green.
- Native Tauri/WebDriverIO E2E green.
- The canonical stylesheet blob must be byte-identical to the legacy stylesheet blob from base `main`.
- `App.css` must differ only by the import path for this stylesheet.
- `code-workspace.css` must differ only in the stale ownership comment and must not grow beyond its frozen baseline.
- Squash merge with the verified exact head SHA.

## Follow-up boundary

Canonicalizing existing live `-vN.css` files is intentionally out of scope. Each one needs import/ownership analysis and regression coverage before rename or consolidation; the current architecture freeze prevents new generations from increasing that baseline.
