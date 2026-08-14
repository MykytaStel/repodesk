# RepoDesk All-Route Performance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every RepoDesk route load only the shell, the active route, and optional tools the user explicitly activates, with manifest-backed JS/CSS budgets that fail the production build on regressions.

**Architecture:** Keep a minimal always-loaded shell, import route styles from the existing lazy route modules, and add first-activation boundaries for Command Palette, bottom-panel tools, and IDE Health details. Enable Vite manifest output and traverse real static/dynamic chunk graphs so budgets describe emitted assets rather than hashed filename guesses.

**Tech Stack:** React 18, TypeScript, Vite 5, CSS cascade layers, Node.js ESM, Playwright, Tauri 2.

## Global Constraints

- Preserve the `base -> legacy -> product -> workbench` cascade order.
- Preserve the current appearance; this slice does not redesign any surface.
- Do not add speculative prefetching or background route loading.
- Shell JavaScript must remain at or below 95 kB gzip.
- Shell CSS must be at or below 18 kB gzip.
- An ordinary route may add at most 35 kB gzip JavaScript and 12 kB gzip CSS.
- Code application JavaScript may add at most 45 kB gzip excluding the separately reported editor vendor graph.
- Xterm JavaScript and CSS must be absent until Terminal activation.
- CodeMirror core must be absent until Code activation.
- Keep the existing 500 kB raw per-chunk ceiling.
- Preserve one PTY session across bottom-panel hide/show and tab changes.
- Use `pnpm --dir apps/desktop ...` from the repository root.

---

### Task 1: Manifest-backed route budget audit

**Files:**
- Create: `apps/desktop/scripts/performance-budget.mjs`
- Create: `apps/desktop/scripts/performance-budget.test.mjs`
- Replace: `apps/desktop/scripts/check-entry-budget.mjs`
- Modify: `apps/desktop/vite.config.ts`
- Modify: `apps/desktop/package.json`

**Interfaces:**
- Produces: `collectStaticGraph(manifest, rootKey): Set<string>`.
- Produces: `measureGraph({ manifest, rootKey, distPath }): { jsGzip: number, cssGzip: number, files: string[] }`.
- Produces: `findChunkBySource(manifest, source): string`.
- Produces: CLI output for shell, route increments, and activated-feature isolation.

- [ ] **Step 1: Write unit tests for manifest traversal and deduplication**

Create a synthetic manifest where `index.html` imports shared vendor chunks, Work and Code are dynamic entries, and two chunks share one stylesheet. Assert that static traversal excludes dynamic imports and counts shared assets once:

```js
import assert from "node:assert/strict";
import test from "node:test";
import { collectStaticGraph, graphFiles } from "./performance-budget.mjs";

const manifest = {
  "index.html": {
    file: "assets/index.js",
    isEntry: true,
    imports: ["_react.js"],
    dynamicImports: ["src/features/work/WorkSurface.tsx", "src/features/code/CodeTab.tsx"],
    css: ["assets/shell.css"],
  },
  "_react.js": { file: "assets/react.js", css: ["assets/shared.css"] },
  "src/features/work/WorkSurface.tsx": {
    src: "src/features/work/WorkSurface.tsx",
    file: "assets/work.js",
    imports: ["_react.js"],
    css: ["assets/work.css", "assets/shared.css"],
  },
  "src/features/code/CodeTab.tsx": {
    src: "src/features/code/CodeTab.tsx",
    file: "assets/code.js",
    imports: ["_react.js"],
    css: ["assets/code.css"],
  },
};

test("static graph excludes dynamic routes", () => {
  assert.deepEqual([...collectStaticGraph(manifest, "index.html")].sort(), ["_react.js", "index.html"]);
});

test("graph files deduplicate shared CSS", () => {
  assert.deepEqual(graphFiles(manifest, new Set(["_react.js", "src/features/work/WorkSurface.tsx"])).sort(), [
    "assets/react.js",
    "assets/shared.css",
    "assets/work.css",
    "assets/work.js",
  ]);
});
```

- [ ] **Step 2: Run the unit tests and verify failure**

Run:

```bash
node --test apps/desktop/scripts/performance-budget.test.mjs
```

Expected: FAIL because `performance-budget.mjs` does not exist.

- [ ] **Step 3: Implement manifest traversal and measurement**

Implement only static `imports` traversal. Expose dynamic chunks by source lookup but do not traverse `dynamicImports` unless that dynamic root is explicitly selected:

```js
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { gzipSync } from "node:zlib";

export function collectStaticGraph(manifest, rootKey) {
  const visited = new Set();
  const visit = (key) => {
    if (visited.has(key)) return;
    const chunk = manifest[key];
    if (!chunk) throw new Error(`Missing manifest chunk: ${key}`);
    visited.add(key);
    for (const dependency of chunk.imports ?? []) visit(dependency);
  };
  visit(rootKey);
  return visited;
}

export function graphFiles(manifest, keys) {
  const files = new Set();
  for (const key of keys) {
    const chunk = manifest[key];
    if (chunk.file) files.add(chunk.file);
    for (const css of chunk.css ?? []) files.add(css);
  }
  return [...files];
}

export function findChunkBySource(manifest, source) {
  const match = Object.entries(manifest).find(([key, chunk]) => key === source || chunk.src === source);
  if (!match) throw new Error(`Missing manifest source: ${source}`);
  return match[0];
}

export function measureGraph({ manifest, rootKey, distPath }) {
  const files = graphFiles(manifest, collectStaticGraph(manifest, rootKey));
  const size = (file) => gzipSync(readFileSync(join(distPath, file))).byteLength;
  return {
    files,
    jsGzip: files.filter((file) => file.endsWith(".js")).reduce((sum, file) => sum + size(file), 0),
    cssGzip: files.filter((file) => file.endsWith(".css")).reduce((sum, file) => sum + size(file), 0),
  };
}
```

- [ ] **Step 4: Enable the manifest and replace the entry-only checker**

Set `build.manifest: true` in `vite.config.ts`. Make `check-entry-budget.mjs` load `dist/.vite/manifest.json`, measure the entry graph, measure route graphs for the source modules in `tabs.tsx`, subtract shell files when reporting route increments, and assert the exact budgets from Global Constraints.

Use these route roots. This list must match every `TabId` in `TAB_IDS`; add a unit test that extracts the quoted ids from `constants.ts` and fails if the two sets drift:

```js
const ROUTES = {
  work: "src/features/work/WorkSurface.tsx",
  code: "src/features/code/CodeTab.tsx",
  changes: "src/features/changes/ChangesTab.tsx",
  history: "src/features/history/HistoryTab.tsx",
  projects: "src/features/projects/ProjectsTab.tsx",
  dashboard: "src/features/dashboard/DashboardTab.tsx",
  tokens: "src/features/tokens/TokensTab.tsx",
  models: "src/features/models/ModelsTab.tsx",
  git: "src/features/git/GitTab.tsx",
  memory: "src/features/knowledge/KnowledgeTab.tsx",
  orchestrate: "src/features/orchestrate/OrchestrateTab.tsx",
  outcomes: "src/features/outcomes/OutcomesTab.tsx",
  playbooks: "src/features/playbooks/PlaybooksTab.tsx",
  "models-cost": "src/features/models-cost/ModelsCostTab.tsx",
  audit: "src/features/audit/AuditTab.tsx",
  settings: "src/features/settings/SettingsTab.tsx",
  system: "src/features/system/SystemTab.tsx",
  debug: "src/features/debug/DebugTab.tsx",
};
```

The product route id `memory` maps to the Knowledge source above; keep the configured key as `memory` in the implementation so it matches `TAB_IDS` and the user-visible navigation contract.

Assert that shell files do not contain manifest roots for `InteractiveTerminal.tsx`, `SemanticCodeEditor.tsx`, `CommandPalette.tsx`, or `IDEHealthPanel.tsx`.

- [ ] **Step 5: Wire the script test into the production build**

Change the desktop build script to:

```json
"build": "tsc && node --test scripts/performance-budget.test.mjs && vite build && node scripts/check-entry-budget.mjs"
```

- [ ] **Step 6: Run the build and observe the expected baseline failure**

Run:

```bash
pnpm --dir apps/desktop run build
```

Expected: unit tests pass; production audit fails because the shell CSS is approximately 27.7 kB gzip and exceeds 18 kB.

- [ ] **Step 7: Commit the audit boundary**

```bash
git add apps/desktop/scripts/performance-budget.mjs apps/desktop/scripts/performance-budget.test.mjs apps/desktop/scripts/check-entry-budget.mjs apps/desktop/vite.config.ts apps/desktop/package.json
git commit -m "test(perf): audit emitted route graphs"
```

### Task 2: Primary-route CSS ownership

**Files:**
- Modify: `apps/desktop/src/app/App.css`
- Create: `apps/desktop/src/features/work/work-route.css`
- Create: `apps/desktop/src/features/changes/changes-route.css`
- Create: `apps/desktop/src/features/history/history-route.css`
- Create: `apps/desktop/src/features/projects/projects-route.css`
- Modify: `apps/desktop/src/features/work/WorkSurface.tsx`
- Modify: `apps/desktop/src/features/changes/ChangesTab.tsx`
- Modify: `apps/desktop/src/features/history/HistoryTab.tsx`
- Modify: `apps/desktop/src/features/projects/ProjectsTab.tsx`
- Modify: `apps/desktop/src/features/history/runs.css`

**Interfaces:**
- Consumes: manifest budget checker from Task 1.
- Produces: one CSS entry attached to each primary lazy route.

- [ ] **Step 1: Add a source-level ownership regression**

Extend `performance-budget.test.mjs` to read `App.css` and assert that route-only imports are absent:

```js
test("shell stylesheet excludes primary route styles", () => {
  const shell = readFileSync(new URL("../src/app/App.css", import.meta.url), "utf8");
  for (const routeOnly of [
    "work.css",
    "work-contract.css",
    "changes-evidence.css",
    "work-hierarchy-v3.css",
    "context-evidence-v2.css",
    "execute-packet-v2.css",
    "observability-v1.css",
    "ai-strategy-v1.css",
    "strategy-feedback-v1.css",
  ]) assert.doesNotMatch(shell, new RegExp(routeOnly.split(".").join("\\.")));
});
```

- [ ] **Step 2: Run the unit test and verify failure**

Run `node --test apps/desktop/scripts/performance-budget.test.mjs`.

Expected: FAIL because `App.css` still imports the route-only styles.

- [ ] **Step 3: Create route CSS entries without changing layer precedence**

Use explicit layer-qualified imports:

```css
/* features/work/work-route.css */
@import "../../app/styles/work.css" layer(legacy);
@import "../../app/styles/work-cockpit.css" layer(legacy);
@import "../../app/styles/work-contract.css" layer(legacy);
@import "../../app/styles/context-evidence-v2.css" layer(workbench);
@import "../../app/styles/execute-packet-v2.css" layer(workbench);
@import "../../app/styles/work-hierarchy-v3.css" layer(workbench);
@import "../../app/styles/observability-v1.css" layer(workbench);
@import "../../app/styles/ai-strategy-v1.css" layer(workbench);
@import "../../app/styles/strategy-feedback-v1.css" layer(workbench);
```

```css
/* features/changes/changes-route.css */
@import "../../app/styles/changes-evidence.css" layer(legacy);
```

```css
/* features/history/history-route.css */
@import "../../app/styles/timeline-table.css" layer(legacy);
@import "./runs.css" layer(legacy);
```

`projects-route.css` owns project registry selectors extracted from route-only blocks in `ide-polish-v1.css` and `workbench-polish-v2.css`. Keep shared ProjectSwitcher selectors in shell ownership.

- [ ] **Step 4: Import each stylesheet from its lazy route module**

Add one static import at the top of each route root:

```ts
import "./work-route.css";
```

Use the corresponding route filename in Changes, History, and Projects. Remove the direct `runs.css` import from `HistoryTab.tsx` because `history-route.css` owns it.

- [ ] **Step 5: Remove migrated imports from `App.css`**

Keep only shell/shared imports in `App.css`. Do not reorder the remaining shell layers.

- [ ] **Step 6: Verify primary routes and the emitted graph**

Run:

```bash
node --test apps/desktop/scripts/performance-budget.test.mjs
pnpm --dir apps/desktop exec playwright test e2e/first-run.spec.ts e2e/work-golden-path.spec.ts e2e/ui-audit.spec.ts
pnpm --dir apps/desktop run build
```

Expected: source ownership test passes; primary route E2E passes; build reports separate route CSS. If shell CSS still exceeds 18 kB, continue with Task 3 rather than raising the budget.

- [ ] **Step 7: Commit primary-route ownership**

```bash
git add apps/desktop/src/app/App.css apps/desktop/src/features/work apps/desktop/src/features/changes apps/desktop/src/features/history apps/desktop/src/features/projects apps/desktop/scripts/performance-budget.test.mjs
git commit -m "perf(css): scope primary route styles"
```

### Task 3: Secondary-route and shared feature CSS ownership

**Files:**
- Modify: `apps/desktop/src/app/App.css`
- Create: `apps/desktop/src/features/orchestrate/orchestrate-route.css`
- Create: `apps/desktop/src/features/knowledge/knowledge-route.css`
- Create: `apps/desktop/src/features/dashboard/dashboard-route.css`
- Create: `apps/desktop/src/features/debug/debug-route.css`
- Create: `apps/desktop/src/features/routing/routing-feature.css`
- Modify: `apps/desktop/src/features/orchestrate/OrchestrateTab.tsx`
- Modify: `apps/desktop/src/features/knowledge/KnowledgeTab.tsx`
- Modify: `apps/desktop/src/features/dashboard/DashboardTab.tsx`
- Modify: `apps/desktop/src/features/debug/DebugTab.tsx`
- Modify: route consumers of `routing-lists.css`
- Split: `apps/desktop/src/app/styles/performance-density-v1.css`
- Split: `apps/desktop/src/app/styles/responsive.css`
- Split: `apps/desktop/src/app/styles/visual-language-2026.css`

**Interfaces:**
- Consumes: route graph budget from Task 1.
- Produces: secondary-route CSS chunks and a shell stylesheet containing no route-exclusive selectors.

- [ ] **Step 1: Add ownership assertions for secondary features**

Extend the source test to reject `orchestrate.css`, `debug.css`, `routing-lists.css`, `knowledge-v2.css`, and all selectors beginning with `.work-`, `.changes-`, `.runs-`, `.orchestrate-`, `.knowledge-`, `.route-`, or `.debug-` in the final shell entry.

- [ ] **Step 2: Run the test and verify failure**

Expected: FAIL on the current `App.css` imports and mixed role-density files.

- [ ] **Step 3: Add route entries**

Use these ownership imports:

```css
/* orchestrate-route.css */
@import "../../app/styles/orchestrate.css" layer(legacy);
```

```css
/* knowledge-route.css */
@import "./knowledge.css" layer(legacy);
@import "./knowledge-v2.css" layer(workbench);
```

```css
/* debug-route.css */
@import "../../app/styles/debug.css" layer(legacy);
```

`dashboard-route.css` owns legacy dashboard/cockpit selectors. `routing-feature.css` owns `routing-lists.css` and is imported only by route modules that render routing controls.

- [ ] **Step 4: Split mixed global files by selector ownership**

Move route-exclusive selector blocks from `performance-density-v1.css`, `responsive.css`, and `visual-language-2026.css` into the relevant route entry. Leave variables, element defaults, shared `.content-grid` density, motion preferences, and shell chrome in the original files.

For responsive blocks, preserve the exact media query while moving the complete selector declaration; do not duplicate declarations between shell and route files.

- [ ] **Step 5: Import route CSS from lazy modules and remove global imports**

Add route imports to Orchestrate, Knowledge, Dashboard, Debug, and each routing consumer. Remove the corresponding `App.css` imports and the previous direct `knowledge.css` import.

- [ ] **Step 6: Verify every registered route renders**

Run:

```bash
pnpm --dir apps/desktop exec playwright test e2e/daily-loop.spec.ts e2e/knowledge-ui.spec.ts e2e/ui-audit.spec.ts
pnpm --dir apps/desktop run build
```

Expected: all selected E2E tests pass; every measured route stays within its route budget; shell CSS is at or below 18 kB gzip.

- [ ] **Step 7: Commit secondary ownership**

```bash
git add apps/desktop/src/app/App.css apps/desktop/src/app/styles apps/desktop/src/features apps/desktop/scripts/performance-budget.test.mjs
git commit -m "perf(css): isolate secondary feature styles"
```

### Task 4: First-activation optional panels

**Files:**
- Modify: `apps/desktop/src/app/App.tsx`
- Modify: `apps/desktop/src/app/WorkbenchBottomPanel.tsx`
- Create: `apps/desktop/src/features/health/IDEHealthPanelGate.tsx`
- Modify: `apps/desktop/src/features/health/IDEHealthIndicator.tsx`
- Modify: `apps/desktop/src/features/health/IDEHealthPanel.tsx`
- Create: `apps/desktop/src/features/health/health-indicator.css`
- Create: `apps/desktop/src/features/health/health-panel.css`
- Modify: `apps/desktop/src/shared/ui/CommandPalette.tsx`
- Modify: `apps/desktop/src/app/styles/terminal.css`
- Modify: `apps/desktop/src/app/styles/task-runner.css`
- Test: `apps/desktop/e2e/trust-polish.spec.ts`

**Interfaces:**
- Produces: `IDEHealthPanelGate(): JSX.Element | null`.
- Produces: first-activation state for palette and bottom panel in `App`.
- Preserves: one mounted `WorkbenchBottomPanel` and one mounted `InteractiveTerminal` after activation.

- [ ] **Step 1: Extend E2E with first-activation assertions**

Add a test that records resource URLs through `page.on("request")`, boots with the bottom panel closed, and asserts no URL contains `InteractiveTerminal`, `vendor-terminal`, or `CommandPalette`. Open each tool, assert its visible contract, close/reopen it, and assert Terminal creates one session only.

Keep the existing IPC assertion:

```ts
await expect.poll(async () =>
  (await recordedCommands(page)).filter((command) => command === "terminal_create").length,
).toBe(1);
```

- [ ] **Step 2: Run the focused test and verify failure**

Run:

```bash
pnpm --dir apps/desktop exec playwright test e2e/trust-polish.spec.ts --workers=1
```

Expected: FAIL because Command Palette and bottom-panel implementation are currently imported by the shell.

- [ ] **Step 3: Lazy-load Command Palette after first open**

Keep the `Command` type as a type-only import. Add:

```ts
const CommandPalette = lazy(() => import("../shared/ui/CommandPalette").then((module) => ({
  default: module.CommandPalette,
})));
const [paletteActivated, setPaletteActivated] = useState(false);
const openPalette = () => {
  setPaletteActivated(true);
  setPaletteOpen(true);
};
```

Route every palette opener through `openPalette`. Render the lazy component only after activation, inside a local Suspense fallback. Import `command-palette-v2.css` from `CommandPalette.tsx`.

- [ ] **Step 4: Lazy-load the bottom panel after first open**

Initialize `bottomPanelActivated` from the persisted open preference. Every UI/event path that opens the panel sets activation first. Keep rendering the lazy `WorkbenchBottomPanel` after activation even when closed.

Import `terminal.css` and `task-runner.css` from `WorkbenchBottomPanel.tsx`; retain Xterm CSS only in `InteractiveTerminal.tsx`.

- [ ] **Step 5: Split IDE Health trigger and panel**

Import `health-indicator.css` from `IDEHealthIndicator.tsx`. Create `IDEHealthPanelGate`:

```tsx
import { lazy, Suspense } from "react";
import { useRecovery } from "./RecoveryProvider";

const IDEHealthPanel = lazy(() => import("./IDEHealthPanel").then((module) => ({
  default: module.IDEHealthPanel,
})));

export function IDEHealthPanelGate() {
  const { panelOpen } = useRecovery();
  if (!panelOpen) return null;
  return (
    <Suspense fallback={null}>
      <IDEHealthPanel />
    </Suspense>
  );
}
```

Import `health-panel.css` from `IDEHealthPanel.tsx`. Replace the eager panel import in App with the gate.

- [ ] **Step 6: Run focused E2E and production budget**

Run:

```bash
pnpm --dir apps/desktop exec playwright test e2e/trust-polish.spec.ts e2e/ide-health.spec.ts
pnpm --dir apps/desktop run build
```

Expected: panel tests pass; optional feature chunks are absent from unrelated shell/route graphs; Terminal session continuity remains green.

- [ ] **Step 7: Commit activated-feature loading**

```bash
git add apps/desktop/src/app apps/desktop/src/features/health apps/desktop/src/shared/ui/CommandPalette.tsx apps/desktop/e2e/trust-polish.spec.ts
git commit -m "perf(desktop): defer optional workspace tools"
```

### Task 5: Route startup matrix and final budget reporting

**Files:**
- Create: `apps/desktop/e2e/route-loading.spec.ts`
- Modify: `apps/desktop/scripts/check-entry-budget.mjs`
- Modify: `docs/RD2_RUNTIME_PERFORMANCE_BUDGET.md`

**Interfaces:**
- Consumes: route graph reports and lazy boundaries from Tasks 1–4.
- Produces: deterministic startup coverage for all 18 registered routes and a documented before/after table.

- [ ] **Step 1: Write the persisted-route startup matrix**

For every id in `TAB_IDS`, set `repodesk.activeTab`, reload, and assert the matching title appears in the `Current workspace location` breadcrumb without opening any unrelated tool. Then assert the route Suspense fallback settles, the workspace surface is non-empty, no page error occurs, and no native browser dialog opens. Keep focused landmark assertions for the five primary routes.

Use one table that covers the complete registry:

```ts
const routes = [
  ["work", "Work"],
  ["code", "Code"],
  ["changes", "Changes"],
  ["history", "Runs"],
  ["projects", "Projects"],
  ["dashboard", "Dashboard"],
  ["tokens", "Tokens"],
  ["models", "Models"],
  ["git", "Git"],
  ["memory", "Knowledge"],
  ["orchestrate", "Orchestrate"],
  ["outcomes", "Outcomes"],
  ["playbooks", "Playbooks"],
  ["models-cost", "Models & Cost"],
  ["audit", "Audit"],
  ["settings", "Settings"],
  ["system", "System Registry"],
  ["debug", "Debug"],
] as const;
```

The second tuple value is the expected breadcrumb title. Use the actual accessible role for each primary surface rather than adding test-only labels. Add a source-level assertion that the route ids in this matrix equal `TAB_IDS`, preventing a newly registered route from silently escaping startup coverage.

- [ ] **Step 2: Run the matrix and repair only ownership regressions**

Run:

```bash
pnpm --dir apps/desktop exec playwright test e2e/route-loading.spec.ts e2e/ui-audit.spec.ts
```

Expected: all routes load with their styles and landmarks. If a route fails visually, move the missing selector to its owner; do not restore the entire stylesheet to `App.css`.

- [ ] **Step 3: Make the budget report complete and stable**

Print a Markdown-compatible table containing shell JS/CSS and all 18 routes' incremental JS/CSS. Sort routes by configured id, not emitted hash. Report editor and terminal vendor graphs separately.

- [ ] **Step 4: Document measured before/after values**

Add a dated section to `RD2_RUNTIME_PERFORMANCE_BUDGET.md` with:

- previous shell: 88.4 kB JS gzip, 27.7 kB CSS gzip;
- final shell JS/CSS;
- each registered route increment, with the five primary routes highlighted;
- confirmation that Terminal and Code editor vendors are activation-scoped.

- [ ] **Step 5: Commit startup coverage and documentation**

```bash
git add apps/desktop/e2e/route-loading.spec.ts apps/desktop/scripts/check-entry-budget.mjs docs/RD2_RUNTIME_PERFORMANCE_BUDGET.md
git commit -m "test(perf): lock all-route loading budgets"
```

### Task 6: Full verification, independent review, and delivery

**Files:**
- Review all changed files.

**Interfaces:**
- Consumes: all previous tasks.
- Produces: clean verified branch and draft PR.

- [ ] **Step 1: Run the full frontend suite**

```bash
pnpm --dir apps/desktop run e2e
pnpm --dir apps/desktop run build
```

Expected: all Playwright tests pass; budget report passes with every configured route.

- [ ] **Step 2: Run the repository gates**

```bash
./scripts/verify-all.sh
cargo clippy --all-targets --all-features -- -D warnings
git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 3: Inspect emitted assets and source ownership**

```bash
rg -n '^@import' apps/desktop/src/app/App.css
rg -n 'vendor-(terminal|editor-core)' apps/desktop/dist/index.html
git status -sb
```

Expected: `App.css` contains shell/shared imports only; `dist/index.html` does not preload terminal/editor vendors; worktree contains only intended changes.

- [ ] **Step 4: Request independent code review**

Review the branch against the design spec with emphasis on manifest traversal correctness, CSS cascade changes, React lazy lifecycle, PTY continuity, and missing route coverage. Fix every high-confidence finding and rerun affected focused tests plus the full build.

- [ ] **Step 5: Push and open a draft PR**

```bash
git push -u origin perf/all-route-loading-budget
```

Create a draft PR targeting `main` that includes the before/after route table and exact verification commands.
