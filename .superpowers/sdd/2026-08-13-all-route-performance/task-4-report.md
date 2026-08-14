# Task 4 Report: First-activation optional panels

## Outcome

Command Palette, the Workbench bottom panel, and the IDE Health panel now load on first activation instead of at closed startup. Once the bottom panel or Terminal has been activated, its mounted instance is retained so the PTY survives panel visibility changes, bottom-tab changes, and primary app-tab changes.

Optional feature CSS is owned by the activating component and remains in the established `legacy` or `workbench` cascade layer. The eager shell graph no longer contains Command Palette, bottom-panel implementation, Terminal/Xterm, or IDE Health panel assets.

## TDD evidence

### Baseline

Before adding Task 4 tests:

```text
pnpm --dir apps/desktop exec playwright test e2e/trust-polish.spec.ts --workers=1
5 passed (4.3s)
```

### RED

Tests were added before production edits.

```text
pnpm --dir apps/desktop exec playwright test e2e/trust-polish.spec.ts --workers=1
2 failed, 5 passed
```

The startup resource assertion reported these eager implementation requests:

```text
http://127.0.0.1:5177/src/features/health/IDEHealthPanel.tsx
http://127.0.0.1:5177/src/shared/ui/CommandPalette.tsx
http://127.0.0.1:5177/src/app/WorkbenchBottomPanel.tsx
```

The lazy-failure test also failed before activation because aborting `CommandPalette.tsx` prevented the shell from booting, proving the palette was an eager shell dependency.

```text
node --test scripts/performance-budget.test.mjs
10 passed, 1 failed
Missing manifest source: src/shared/ui/CommandPalette.tsx
```

This proved there was no standalone palette activation root in the emitted manifest.

### GREEN

```text
pnpm --dir apps/desktop exec playwright test e2e/trust-polish.spec.ts --workers=1 -g "optional workspace tools"
1 passed (2.6s)

pnpm --dir apps/desktop exec playwright test e2e/trust-polish.spec.ts e2e/ide-health.spec.ts --workers=1
10 passed (6.2s)

node --test scripts/performance-budget.test.mjs
11 passed, 0 failed

pnpm --dir apps/desktop exec tsc --noEmit
exit 0

pnpm --dir apps/desktop run build
exit 0

pnpm --dir apps/desktop run e2e
85 passed (18.2s)
```

The aggregate build included TypeScript, all 11 performance/manifest unit tests, Vite production output, and the shell/route/activated-feature audit.

## Runtime resource and PTY evidence

The new Playwright coverage records every requested URL before navigation and asserts closed startup requests none of these contracts:

- `CommandPalette` / `command-palette-v2.css`
- `WorkbenchBottomPanel`
- `InteractiveTerminal`, `vendor-terminal`, `@xterm`, `xterm.css`
- `terminal.css` / `task-runner.css`
- `IDEHealthPanel.tsx` / `health-panel.css`

Closed startup also records zero `terminal_create` IPC calls.

Covered activation paths:

- Command Palette: `Meta+K`, activity-rail button, and `open-command-palette` Tauri event.
- Bottom panel: `repodesk:bottom-panel-tab` event, `Meta+J`, activity-rail button, palette `Toggle bottom panel`, and palette `Run configured checks` action.
- Persisted startup: `repodesk.bottomPanelOpen=1` activates and shows the panel without creating a Terminal session.
- IDE Health: the always-available health indicator activates the lazy panel.

The first Terminal activation records exactly one `terminal_create`. The count remains exactly one after closing/reopening the bottom panel, switching Output/Terminal tabs, navigating Code/Work primary tabs, toggling through the palette, and opening the panel after checks. The same PID remains visible. Reopening optional features does not issue another implementation-resource request.

Aborting the first Command Palette implementation request now leaves the shell bootable until activation, then surfaces the existing visible app error boundary with `RepoDesk hit an unexpected error` and a `Reload app` recovery action instead of a blank page.

## Emitted manifest and build sizes

The emitted manifest now has standalone roots for:

- `src/shared/ui/CommandPalette.tsx`
- `src/app/WorkbenchBottomPanel.tsx`
- `src/app/InteractiveTerminal.tsx`
- `src/features/health/IDEHealthPanel.tsx`

The manifest test verifies each root implementation is absent from the eager shell graph. It also verifies `.cmdk-panel-v2`, `.interactive-terminal`, `.task-runner-panel`, and `.ide-health-overlay` load with their feature roots and are absent from shell CSS, while `.ide-health-indicator` remains in shell CSS.

Production output:

| Asset | Raw | Gzip |
| --- | ---: | ---: |
| Eager shell JavaScript graph | — | 83.0 kB |
| Eager shell CSS graph | — | 13.0 kB |
| `CommandPalette` JavaScript | 3.80 kB | 1.70 kB |
| `CommandPalette` CSS | 3.14 kB | 0.98 kB |
| `WorkbenchBottomPanel` JavaScript | 12.44 kB | 4.10 kB |
| `WorkbenchBottomPanel` CSS | 9.53 kB | 2.03 kB |
| `InteractiveTerminal` JavaScript | 4.99 kB | 2.08 kB |
| `vendor-terminal` JavaScript | 334.02 kB | 84.74 kB |
| `vendor-terminal` CSS | 5.24 kB | 1.92 kB |
| `IDEHealthPanel` JavaScript | 7.04 kB | 2.15 kB |
| `IDEHealthPanel` CSS | 8.09 kB | 1.93 kB |

All 18 route increments remain within their JavaScript and CSS budgets. The editor and terminal vendor graphs remain absent from the eager shell.

## Files

Modified:

- `apps/desktop/e2e/trust-polish.spec.ts`
- `apps/desktop/scripts/check-entry-budget.mjs`
- `apps/desktop/scripts/performance-budget.test.mjs`
- `apps/desktop/src/app/App.css`
- `apps/desktop/src/app/App.tsx`
- `apps/desktop/src/app/WorkbenchBottomPanel.tsx`
- `apps/desktop/src/app/styles/command-palette-v2.css`
- `apps/desktop/src/app/styles/task-runner.css`
- `apps/desktop/src/app/styles/terminal.css`
- `apps/desktop/src/features/health/IDEHealthIndicator.tsx`
- `apps/desktop/src/features/health/IDEHealthPanel.tsx`
- `apps/desktop/src/shared/ui/CommandPalette.tsx`

Created:

- `apps/desktop/src/features/health/IDEHealthPanelGate.tsx`
- `apps/desktop/src/features/health/health-indicator.css`
- `apps/desktop/src/features/health/health-panel.css`

Removed after the ownership split:

- `apps/desktop/src/features/health/health.css`

## Self-review

- All palette openers route through `openPalette`; the global modal guard and toggle-close shortcut behavior remain unchanged.
- All panel open/toggle paths activate before opening. The app-level bottom-tab event handler preserves the requested first tab even before the lazy panel mounts.
- `WorkbenchBottomPanel` remains mounted after first activation, including while hidden. `InteractiveTerminal` remains mounted after its first activation.
- The `Command` and `BottomPanelTab` imports used only as types are type-only.
- Lazy feature imports do not add speculative preload calls.
- Optional CSS declares its original `legacy` or `workbench` layer locally, preserving cascade order when loaded later.
- A normalized before/after declaration comparison reports `declarations preserved` for health, Terminal, Task Runner, and Command Palette CSS; only ownership and layer wrappers changed.
- Route CSS ownership and `timeline-table.css` were not changed.
- `git diff --check` is clean, and the worktree contained no unrelated pre-existing changes at task start.

## Concerns

No blocking concerns. A failed optional chunk currently escalates to the existing root app error boundary rather than a feature-local retry boundary; this is visible and recoverable and matches the scoped requirement without introducing unrelated trust-polish behavior.

## Fix Round 1

### Findings addressed

1. The production entry audit now compares every activated feature's complete emitted static graph with the complete eager shell graph. It no longer checks only the activation root file and root CSS.
2. Persisted-bottom-panel startup now records requests and proves Terminal/Xterm assets remain absent until the user selects Terminal, then proves those resources load and exactly one PTY is created.

### RED evidence

The graph test was added before the audit helper or checker changed:

```text
node --test --test-name-pattern="activation graph audit" scripts/performance-budget.test.mjs
not ok 1 - activation graph audit rejects an eager child while allowing only explicit shell platform assets
Expected: ["assets/optional-child.js"]
Actual:   []
1 failed
```

The synthetic feature root itself remained lazy. Its child was part of both the feature and shell graphs, exposing the exact root-only blind spot.

The persisted-startup request assertions were also added before production changes:

```text
pnpm --dir apps/desktop exec playwright test e2e/trust-polish.spec.ts --workers=1 -g "persisted-open startup"
1 passed (2.8s)
```

This test was green immediately because persisted startup already deferred `InteractiveTerminal` correctly; the finding was a coverage gap, so no runtime production change was needed for it.

### Implementation and allowlist rationale

`activatedFeatureEagerFiles` computes all files in the feature static graph and all files in the shell static graph, then returns their unexpected overlap. The production checker uses it for bottom panel, Terminal, Command Palette, and IDE Health.

The exact allowed overlap is:

- Direct `index.html` entry JS and CSS only. Vite links shared first-party providers, hooks, and API utilities used by optional features back to the shell entry, so the entry's own assets legitimately appear in their static graphs.
- Direct assets named `vendor-react`, `vendor-query`, and `vendor-tauri` only. These are platform libraries already required by the shell and used by the activated features.

The allowlist is deliberately non-transitive. Dependencies of the shell entry and dependencies of allowed vendor chunks are not allowed automatically. The strengthened synthetic fixture places an arbitrary child JS and CSS behind an allowed `vendor-react` chunk and verifies both still fail the overlap audit. `vendor-terminal` and `vendor-editor-core` are not allowlisted; their existing dedicated shell-isolation checks remain unchanged.

The emitted-manifest integration test applies the same full-graph contract to all four activation roots and continues to check their feature CSS selectors against eager shell CSS.

### GREEN evidence

```text
node --test --test-name-pattern="activation graph audit" scripts/performance-budget.test.mjs
1 passed

pnpm --dir apps/desktop exec playwright test e2e/trust-polish.spec.ts --workers=1 -g "persisted-open startup"
1 passed (1.9s)

node --test scripts/performance-budget.test.mjs
12 passed, 0 failed

pnpm --dir apps/desktop exec playwright test e2e/trust-polish.spec.ts e2e/ide-health.spec.ts --workers=1
10 passed (8.0s)

pnpm --dir apps/desktop exec tsc --noEmit
exit 0

pnpm --dir apps/desktop run build
exit 0
```

The aggregate build reran TypeScript, all 12 manifest/performance tests, Vite production output, and the production entry audit. Final audit measurements remain:

- Shell: 83.0 kB JavaScript / 13.0 kB CSS gzip.
- Editor vendor graph: 113.4 kB JavaScript / 0.0 kB CSS gzip, absent from shell.
- Activated feature audit: bottom panel, Terminal, Command Palette, and IDE Health all have zero unexpected shell overlap.
- All 18 route increments remain within budget.

### Files changed in Fix Round 1

- `apps/desktop/scripts/performance-budget.mjs`
- `apps/desktop/scripts/check-entry-budget.mjs`
- `apps/desktop/scripts/performance-budget.test.mjs`
- `apps/desktop/e2e/trust-polish.spec.ts`
- `.superpowers/sdd/2026-08-13-all-route-performance/task-4-report.md`

### Self-review

- The helper compares full static file graphs and returns every unexpected overlapping JS or CSS asset.
- Allowed root/chunk files are direct only; no dependency graph is transitively exempted.
- The synthetic mutation keeps the wrapper lazy and makes a child eager through an allowed platform chunk; the test catches both child JS and CSS.
- The real emitted-manifest test covers all four activation roots.
- Persisted-open startup waits for network idle before asserting zero Terminal/Xterm resources, then polls for positive resource loading after Terminal selection and retains the exact-one `terminal_create` assertion.
- Terminal/editor vendor isolation and route/CSS budgets were not weakened.
- No runtime component, route CSS, or `timeline-table.css` file changed in this round.
- `git diff --check` is clean.

No blocking concerns remain. Fix Round 1 will be committed as `fix(perf): audit full activation graphs`.
