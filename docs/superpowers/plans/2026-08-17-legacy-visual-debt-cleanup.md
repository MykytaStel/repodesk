# Legacy Visual Debt Cleanup Plan

## Goal

Finish the Cut F visual-debt cleanup after Changes, Work, Runs, Projects, and Code convergence by retiring the remaining legacy visual-generation stylesheet and making the architecture gate reject legacy `*-polish.css` and `-vN.css` files anywhere under the desktop source tree.

## Scope

- Remove `apps/desktop/src/features/code/code-editor-polish.css` only after proving it is legacy/dead visual debt.
- Generalize the architecture ratchet so existing tracked desktop source paths matching `*-polish.css` or `-vN.css` fail the cleanup contract.
- Preserve the existing changed-file freeze that already rejects new visual generations and feature CSS growth.
- Keep Work's canonical hierarchy contract and all Changes/Work/Runs/Projects/Code semantic contracts intact.
- No runtime UI behavior, CSS replacement, backend/Rust, domain, or routing changes.

## TDD sequence

1. RED: add a design-system ratchet test that requires a generic legacy visual-debt cleanup evaluator and asserts the repository has no tracked legacy visual-generation stylesheets.
2. Verify the RED failure is caused by the missing generic contract / existing `code-editor-polish.css` debt.
3. GREEN: implement the smallest generic path classifier + repository cleanup contract in `scripts/check-source-architecture.mjs` and wire it into `runArchitectureRatchet()`.
4. Delete the dead `apps/desktop/src/features/code/code-editor-polish.css` file.
5. Verify the design-system ratchet turns green without changing runtime production surfaces.

## Merge gate

On the exact final head require:

- Architecture Ratchet green.
- Full CI green.
- Native Tauri/WebDriverIO E2E green.
- Final diff limited to ratchet/test/plan plus deletion of the dead stylesheet.
- Squash merge with the verified exact head SHA.
