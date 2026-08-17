# Legacy Visual Debt Cleanup Plan

## Goal

Finish the Cut F polish-layer cleanup after Changes, Work, Runs, Projects, and Code convergence by retiring the remaining tracked `*-polish.css` stylesheet and making the architecture gate enforce a zero-polish baseline across desktop source.

## Evidence-driven scope

- `apps/desktop/src/features/code/code-editor-polish.css` is tracked but has no repository references/imports, so it is dead visual debt and can be removed without replacing runtime styles.
- Existing tracked `-vN.css` stylesheets are **not** removed or globally failed in this PR. The first generic RED attempt proved multiple versioned stylesheets remain and may still be live (`ai-strategy-v1.css`, `command-palette-v2.css`, `knowledge-v2.css`, and others).
- The existing changed-file freeze already rejects **new** `-vN` visual generations and new `*-polish.css` stylesheets, so current versioned files remain a frozen baseline to canonicalize in separate, evidence-backed slices.
- Preserve feature CSS growth limits, Work canonical hierarchy ownership, and all Changes/Work/Runs/Projects/Code semantic contracts.
- No runtime UI behavior, CSS replacement, backend/Rust, domain, or routing changes.

## TDD sequence

1. RED: require a generic legacy polish cleanup evaluator in the design-system ratchet. Confirm failure because the contract does not exist.
2. Implement the generic scanner and observe the broader `-vN` experiment. Use that failure as evidence to reject a blanket zero-versioned rule rather than deleting potentially live styles.
3. Refine the invariant to tracked `*-polish.css` only. Confirm the ratchet fails on exactly `apps/desktop/src/features/code/code-editor-polish.css`.
4. GREEN: delete that proven-dead stylesheet while keeping the generic zero-polish contract wired into `runArchitectureRatchet()`.
5. Verify the design-system ratchet and complete application gates turn green without runtime production-surface changes.

## Merge gate

On the exact final head require:

- Architecture Ratchet green.
- Full CI green.
- Native Tauri/WebDriverIO E2E green.
- Final diff limited to ratchet/test/plan plus deletion of the dead stylesheet.
- Squash merge with the verified exact head SHA.

## Follow-up boundary

Canonicalizing existing live `-vN.css` files is intentionally out of scope. Each one needs import/ownership analysis and regression coverage before rename or consolidation; the current architecture freeze prevents new generations from increasing that baseline.