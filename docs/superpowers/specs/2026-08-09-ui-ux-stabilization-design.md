# RepoDesk UI/UX Stabilization Design

## Goal

Make the current RepoDesk desktop interface consistently readable, correctly aligned, and predictable without redesigning the product workflow. The approved visual direction is balanced IDE density: compact enough for engineering work, with important interface text at 12 px or larger and clear hierarchy between primary and secondary content.

## Scope

This stabilization covers the shared desktop shell and every primary product tab currently reachable from it. It includes editor geometry, scrolling, typography, spacing, controls, empty/loading/error/disabled states, responsive behavior, dark and light themes, keyboard focus, and repository hygiene for local generated artifacts.

The work preserves existing product behavior and information architecture. It does not introduce a new editor engine, replace the navigation model, or redesign backend workflows.

## Design Principles

1. One component owns each visual behavior. Scroll position, selection, focus, and active state must not be represented by competing layers.
2. Shared tokens and shared controls define the baseline. Feature CSS may specialize layout but must not override global geometry with conflicting late imports.
3. Dense does not mean tiny. Primary text and controls use a readable 12–14 px range; smaller text is limited to nonessential metadata and must remain legible.
4. Empty states use the available space intentionally without becoming full-width empty panels.
5. Every state is visually explicit: loading, empty, disabled, focused, active, success, warning, and error.
6. The interface must remain usable in dark and light themes and at both normal and narrow desktop window widths.

## Editor and Gutter

The textarea remains the only scroll surface. The gutter is a clipped visual companion and never shows or owns a vertical or horizontal scrollbar.

Line numbers and source text share authoritative constants for top padding, line height, and font metrics. Scrolling applies the textarea's exact `scrollTop` to the gutter's visual track. Opening a requested line updates the textarea first and then synchronizes the gutter.

The selected line number is rendered exactly once. The normal line-number track must not remain visible beneath a translucent duplicate. The selected state may use a solid token-based background or a single styled line-number representation, but it cannot rely on two visible copies aligning perfectly.

Acceptance criteria:

- no scrollbar appears inside or over the line-number gutter;
- the only visible scrollbar belongs to the source area;
- every visible number aligns with its corresponding source line at the start, middle, and end of long files;
- horizontal source scrolling never moves the gutter;
- selecting a line produces one crisp number with no ghosting;
- opening a line programmatically places the caret and active number on the same row;
- the behavior remains correct with one-line files, trailing newlines, long lines, and narrow windows.

## Shared Visual Foundation

Foundation tokens become the source of truth for readable text sizes, control heights, spacing, borders, focus rings, muted contrast, and disabled opacity. Late feature-polish files may consume these tokens but must not redefine the same component at incompatible sizes.

Primary content uses 12–14 px type. Page titles remain visually prominent without consuming excessive vertical space. Secondary metadata may be smaller only where it is not required to complete an action. Buttons and form fields maintain clear hit areas, visible keyboard focus, and sufficient disabled-state contrast.

CSS ownership will be simplified where current import order creates conflicting definitions. The change remains targeted: consolidate rules for touched components rather than performing an unrelated stylesheet rewrite.

## Knowledge and Work Surfaces

Project Knowledge keeps the approved balanced layout:

- a compact page header with a visible primary action;
- a readable filter strip whose selected count and label do not overlap or ghost;
- a bounded empty state aligned with the content column;
- a proposal form with a sensible maximum width, balanced category/title columns, readable labels, and an unambiguous disabled submit button;
- responsive stacking when the window cannot support two columns.

The Work surface uses the same hierarchy and density. Scope and Project Knowledge navigation must not become oversized empty horizontal bands. Empty task states communicate the next action without leaving misleading interactive-looking regions or excessive dead space.

## Full UI/UX Audit

Each reachable primary tab will be reviewed in the running desktop-compatible frontend for:

- clipping, overlap, overflow, and accidental scrollbars;
- inconsistent font sizes, control heights, spacing, and content widths;
- missing or confusing empty, loading, error, disabled, selected, and focus states;
- unclear primary actions and weak visual hierarchy;
- narrow-window reflow;
- dark- and light-theme contrast;
- keyboard reachability and visible focus;
- console errors caused by rendering or interaction.

Findings will be fixed through shared primitives when multiple screens share the cause. Feature-local fixes will be used only for genuinely local problems. New functionality and unrelated product changes are outside this stabilization.

## Repository Hygiene

The repository ignore rules will be validated against actual generated files. `.superpowers/` is a confirmed local brainstorming artifact and will be ignored. Existing rules for build outputs, dependency directories, logs, databases, OS metadata, test reports, local RepoDesk data, and debug bundles will be retained.

Before adding any pattern, tracked files and documented project assets will be checked to avoid hiding source, migrations, lockfiles, test fixtures, icons, or configuration examples that belong in Git. Acceptance requires a clean `git status` after normal build and UI-test workflows, excluding intentional source changes.

## Testing Strategy

Regression coverage is test-first where the behavior can be expressed automatically:

- component tests assert the editor's single-scroll-owner and single-active-number structure;
- geometry helpers, if extracted, receive unit coverage for line placement and offsets;
- Playwright coverage checks gutter overflow, active-number uniqueness, scroll synchronization, responsive form layout, and representative empty states;
- existing frontend build, lint/type checks, and relevant E2E smoke tests remain green.

Visual verification uses representative normal and narrow desktop sizes in both dark and light themes. Long-file editor checks cover the beginning, middle, and end of the document. Console output is inspected during interaction runs.

## Delivery Sequence

1. Add regression tests for editor structure and scrolling, then fix gutter ownership, alignment, and active-number rendering.
2. Normalize the shared typography, spacing, controls, focus, muted, and disabled tokens needed by the affected screens.
3. Stabilize Knowledge and Work layouts and state presentation.
4. Audit remaining primary tabs and correct issues through shared or local ownership as appropriate.
5. Harden responsive and theme behavior and run visual regression checks.
6. Update `.gitignore`, verify generated artifacts stay untracked, and run the project verification gates.

## Completion Criteria

The work is complete only when the reported editor defects are absent in runtime verification, all primary tabs have been reviewed, found in-scope defects are fixed, dark/light and normal/narrow checks pass, automated gates are green, and normal local workflows no longer expose generated artifacts as Git changes.
