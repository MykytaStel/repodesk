# Workbench Interaction Contract Design

**Date:** 2026-08-18

## Problem

RepoDesk now has a coherent top-level engineering lifecycle (`Work → Code → Changes → Runs → Projects`) and a shared semantic visual language, but side surfaces still use multiple local interaction dialects. The shell has a workspace sidebar, a workspace inspector, and a bottom panel. Work has its own command rail and inspector. Code has an explorer plus two right-side drawers. Changes has an inline manifest surface plus a findings drawer. These surfaces disagree on naming, close behavior, keyboard behavior, focus restoration, and ownership.

This fragmentation creates three product failures:

1. Dense routes become collections of nested panels instead of a predictable engineering workbench.
2. The same user action has different interaction rules depending on the route.
3. Strong evidence/governance semantics are visually weakened by presentation mechanics that look like generic AI-IDE chrome.

## Product Principle

RepoDesk is an evidence-native engineering workspace for controlled software change. Agents, providers, and models are infrastructure. The primary product object is a governed change and its evidence chain.

The interaction model must therefore make engineering state easier to inspect than UI state.

## Canonical Surface Types

RepoDesk has exactly six surface types:

1. **Activity Rail** — persistent global navigation and shell commands.
2. **Navigator** — the single left-side structural pane for route navigation/selection.
3. **Workspace** — the single primary working area.
4. **Inspector** — the single right-side contextual pane for the current selection/evidence.
5. **Bottom Panel** — Problems, Tasks/Checks, Output, and Terminal.
6. **Dialog / Popover** — transient modal or anchored interactions.

`sidebar` and `drawer` are no longer product-level terms for structural workbench panes.

## Surface Laws

1. One concept has one owning surface.
2. One selection has at most one Inspector.
3. One action scope has at most one visually dominant primary action.
4. Structural panes are not drawers.
5. Evidence is mutated only by its authority surface.
6. Activity is not trust.
7. Trust must explain why.
8. Stale evidence can never appear positive.
9. Compatibility inputs are not reviewed Engineering Knowledge.
10. Agents produce changes; RepoDesk governs changes.

## Navigator Contract

Desktop behavior:

- Docked structural pane.
- `Cmd/Ctrl+B` toggles it.
- Its open state may persist across route changes.
- Route changes replace Navigator content rather than layering another left pane.
- It does not close on `Escape` in docked desktop mode.

Compact/overlay behavior is a future responsive implementation of the same semantic object. When the Navigator is rendered as an overlay it must close on `Escape`, outside click, explicit close, or completed navigation.

## Inspector Contract

- There is one logical Inspector slot.
- A new inspector request replaces the previous inspector content.
- Re-triggering the same inspector request toggles it closed.
- An explicit close button is always present.
- `Escape` closes the Inspector when no modal dialog owns `Escape`.
- Closing restores focus to the element that opened it when that element is still connected.
- Route changes close route-local inspector content unless the new route explicitly maps the same selection.
- Inspector state must never nest another workbench Inspector or Drawer.

Initial migration targets:

- shell `WorkspaceInspector`
- Work `work-inspector-pane`
- Code `RepositoryIntelligenceDrawer`
- Code `code-insights-drawer`
- Changes `changes-findings-drawer`
- Changes Safe Commit / governance evidence

## Bottom Panel Contract

- `Cmd/Ctrl+J` toggles the panel.
- The panel has an explicit local close control.
- Global `Escape` does not close it because Terminal/editor workflows own `Escape` locally.
- Panel tab changes do not implicitly close the panel.
- Existing persisted open state remains supported.
- Resizing and persisted height are deferred to a dedicated follow-up slice; this foundation must not block that work.

## Dialog Contract

The existing shared dialog behavior is the reference for transient interaction discipline:

- focus trap;
- `Escape` close where safe;
- focus restoration;
- backdrop behavior;
- `aria-modal` semantics.

Workbench structural surfaces do not become modal dialogs just to inherit these mechanics.

## Action Ownership Contract

A route/section may expose one primary action for the next meaningful state transition. Repeated access to the same action may exist in the command palette, but not as multiple visually competing primary CTAs in the same visible scope.

Form behavior must be one of two explicit models:

- **Auto-save** — editing persists immediately and the UI reports `Saving…` / `Saved`; there is no redundant `Save changes` button.
- **Staged-save** — editing is local until explicit `Save` or `Cancel`.

A form must never mix the two models.

## Error Contract

User-visible structured errors must go through the canonical error normalizer. Direct `String(error)` coercion is prohibited in migrated product surfaces because structured Tauri errors otherwise render as `[object Object]`.

The architecture ratchet must catch newly introduced user-visible `String(error)` patterns in the desktop frontend. Existing occurrences are a burn-down list, not accepted design.

## Accessibility Contract

- Activity Rail buttons expose pressed state for toggled structural surfaces.
- Navigator and Inspector have stable accessible labels.
- Inspector close controls have explicit `aria-label` text.
- Inspector `Escape` handling must not override an open modal dialog.
- Focus returns to the opener after Inspector close when possible.
- No inaccessible click-only outside-close behavior is required for docked desktop panes.

## Foundation Slice

The first implementation slice is intentionally architectural rather than route-redesigning. It will:

1. Introduce canonical workbench surface terminology/types and a shared `WorkbenchInspectorSurface` primitive.
2. Rename shell-level sidebar semantics to Navigator semantics without changing the persisted storage key format.
3. Give the shell Inspector an explicit local close control, `Escape` behavior, and focus restoration.
4. Preserve bottom-panel `Cmd/Ctrl+J` behavior and explicitly exclude global `Escape` dismissal.
5. Add E2E coverage for Navigator/Inspector/Bottom Panel keyboard and close contracts.
6. Add an architecture ratchet preventing new structural `drawer` terminology and new user-visible `String(error)` coercion in migrated workbench code.
7. Normalize the existing shell Inspector structured error rendering.

Route-specific migration follows in independent PRs so each route can be validated visually and behaviorally without a giant shell rewrite.

## Follow-up Slices

### Work + Code surface migration

- Work command rail becomes route Navigator content.
- Work Inspector adopts the canonical Inspector surface.
- Code explorer becomes route Navigator content.
- Repository Intelligence and Findings become mutually exclusive Inspector content.

### Changes authority + Inspector migration

- Safe Commit Manifest and Findings use the same Inspector slot.
- Duplicate Manifest toggles are removed.
- Changes remains the authority surface for review/verification/acceptance mutations.

### Runs authority cleanup

- Runs becomes read-only forensic evidence.
- `Link proof` mutation moves to Changes.
- Provider outcomes and raw audit become evidence views/details rather than competing sibling products.

### Projects / Knowledge composition

- Engineering Knowledge is separated from compatibility Sources.
- Imports/AGENTS/CLAUDE/Cursor/Copilot inputs are Sources until explicitly promoted.
- Duplicate `Add knowledge` ownership is removed.
- Library uses master/detail composition instead of stacked full-width cards.

### Density and product identity

- Build a reusable Change Passport / Why interaction on top of the converged Inspector.
- Remove redundant explanatory copy, card nesting, headers, and borders after the interaction hierarchy is stable.

## Non-goals for the Foundation Slice

- No provider/model behavior changes.
- No backend evidence semantics changes.
- No complete Projects or Settings redesign in the foundation PR.
- No route-specific layout rewrite before the canonical surface mechanics exist.
- No new UI framework dependency.
- No visual generation/versioned CSS layers.
