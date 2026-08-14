# RepoDesk All-Route Performance Design

## Goal

Make every RepoDesk route load only the shell, the active route, and the optional features the user has explicitly opened. Preserve current behavior, visual hierarchy, keyboard access, PTY continuity, and evidence contracts while making bundle regressions fail the production build.

## Current Baseline

The production build currently emits:

- 88.4 kB gzip of initial JavaScript across the entry and preloaded vendor files;
- one 172.7 kB raw / 27.7 kB gzip global stylesheet;
- lazy Xterm and CodeMirror vendor chunks;
- route-level JavaScript chunks for the primary and secondary surfaces.

The remaining structural cost is that `App.css` imports feature styling for Work, Changes, Orchestrate, Debug, Terminal, Task Runner, Knowledge, context evidence, strategy, observability, and IDE Health before those surfaces are needed. Several closed optional panels are also part of the eager application graph.

## Loading Model

RepoDesk uses three loading levels.

### Shell

The shell is always available and owns only:

- foundation variables and themes;
- application frame, activity rail, title bar, breadcrumbs, and drawers;
- shared buttons, fields, pills, notices, loaders, empty states, and dialogs;
- responsive shell behavior;
- shared focus and accessibility states.

The shell must not own selectors that exist only for one route or one optional panel.

### Route

Every primary and secondary route imports its own styles from the same lazy module boundary as its React component. Opening a route loads its route JavaScript and route CSS together.

The initial active route may come from persisted navigation state. Therefore the contract applies equally to `Work`, `Code`, `Changes`, `Runs`, `Projects`, and every command-palette-only route; it is not limited to the default Work screen.

Shared feature styling may be reused through a small explicit feature stylesheet, but it must not be moved back into the shell only to avoid duplicate imports.

### Activated Feature

Closed optional tools do not belong in the initial graph. The following receive explicit first-activation boundaries:

- Interactive Terminal;
- Task Runner and structured task UI;
- IDE Health detail panel;
- Command Palette;
- other hidden overlays whose implementation or styling is not required to render their closed trigger.

After first activation, stateful tools remain mounted when their existing product contract requires continuity. In particular, hiding the bottom panel or selecting another bottom-panel tab must not destroy an active PTY.

## CSS Ownership

`App.css` remains the cascade entry point for shell-only layers. Feature styles move next to their owner and are imported from the lazy route or activated-feature module.

The existing cascade order remains authoritative:

```text
base -> legacy -> product -> workbench
```

Route and feature styles must declare the appropriate layer locally so moving an import does not change specificity or visual precedence. The optimization must not be implemented by duplicating selectors, raising specificity, or appending another global override layer.

The migration should remove obsolete global imports and consolidate only selectors encountered in the moved ownership boundary. Unrelated visual redesign is out of scope.

## JavaScript Boundaries

The route registry remains based on `React.lazy`. Optional panels use lazy module boundaries at their first open event.

The application shell may retain cheap trigger components and state coordination. Heavy implementation modules, route-specific hooks, editors, terminals, and panel bodies must remain behind their owning boundary.

No speculative background preload is introduced. Stable idle state remains quiet: opening RepoDesk does not fetch routes or optional tools merely because they might be used later.

## Budget Contract

The production budget checker reads Vite's manifest and traverses emitted imports and dynamic imports. It reports each route in terms of shell cost and route increment instead of relying only on hashed filenames.

Initial targets:

- shell JavaScript: at most 95 kB gzip;
- shell CSS: at most 18 kB gzip;
- ordinary route increment: at most 35 kB gzip JavaScript and 12 kB gzip CSS;
- Code application increment: at most 45 kB gzip JavaScript, excluding the separately reported editor vendor boundary;
- Xterm JavaScript and CSS absent until Terminal activation;
- CodeMirror core absent until Code activation;
- activated-feature chunks absent from unrelated route graphs.

If measured shell ownership cannot reach the CSS target without breaking a shared visual contract, the implementation must identify the exact shared selectors and revise the target in the spec before weakening the build gate.

The existing 500 kB raw per-chunk ceiling remains in force.

## Runtime and Failure Behavior

Every lazy boundary renders a bounded, route-appropriate fallback. A failed dynamic import must surface through the existing tab or panel error boundary rather than leaving a blank workspace.

Loading a route must not trigger unrelated IPC work. Moving component boundaries must preserve current React Query cache keys, mutation invalidation, recovery listeners, and terminal lifecycle.

Styles must arrive with their owning module so the application does not display a persistent unstyled state. Short browser-level module loading is covered by the existing Suspense fallback.

## Verification

The implementation starts by extending the production audit and observing it fail against the current global CSS/eager-feature graph.

Automated verification covers:

- manifest graph assertions for shell, every primary route, representative secondary routes, and activated features;
- default Work startup and persisted startup into each primary route;
- no eager Terminal creation or Xterm asset request;
- one Terminal creation after activation and PTY continuity across tab and panel hide/show;
- Command Palette and IDE Health loading on first open;
- route navigation without missing styles or inaccessible loading states;
- existing wide and narrow UI audits;
- full Playwright, TypeScript/Vite production build, Rust workspace verification, strict Clippy, secret scan, and diff check.

The final handoff records before/after shell JS and CSS gzip sizes plus the incremental cost of every primary route.

## Non-Goals

- no visual redesign;
- no speculative route prefetching;
- no service worker or persistent web cache;
- no replacement of React, Vite, CodeMirror, or Xterm;
- no weakening of PTY, review, evidence, security, or accessibility behavior;
- no dependency upgrades as part of the performance slice.
