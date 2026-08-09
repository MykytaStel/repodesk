# RepoDesk 2 — Performance + UI Density v1

This slice follows the Runtime Performance Budget v0 and applies the same principle to the two highest-frequency engineering surfaces:

> Stable state should be quiet. The current decision should dominate the screen.

## Scope

### Runtime

- remove the remaining fixed 4-second Work phase polling;
- remove the remaining fixed 4-second Changes engineering polling;
- replace broad Work mutation cache invalidation with explicit query domains;
- stop Changes from launching RepoPilot automatically when the surface mounts;
- add bounded aggregate runtime metrics for the shared `callCommand` transport.

### UI density

- Work becomes a vertical decision flow rather than a dashboard of explanatory cards;
- Changes becomes a file-list + diff master/detail workspace;
- governance remains visible as one commit-gate line and expands on demand;
- RepoPilot findings are explicitly requested and displayed in an on-demand drawer;
- routine `allowed / in scope` state is not repeated as a badge on every file;
- Runs evidence sections become flatter, separator-driven document sections.

## Work runtime contract

`work_phase_state` no longer runs on a fixed timer.

The Work surface now updates through:

1. initial query;
2. a short stale window;
3. window-focus refresh;
4. the result returned by Work mutations;
5. explicit domain invalidation after actions that can change adjacent state.

Mutation invalidation is deliberately scoped:

- context/action work → `work`, `runs`;
- agent execution → `work`, `git`, `code`, `runs`;
- review/import → `work`, `git`, `code`, `runs`;
- verification → `work`, `runs`;
- bounded commit → `work`, `git`, `code`, `runs`.

Provider health, token analytics and system discovery are not woken by those mutations.

## Changes runtime contract

The Changes engineering aggregate no longer polls every four seconds.

It refreshes from:

- query cache;
- short stale window;
- focus refresh;
- explicit Refresh;
- Work/domain invalidation after engineering mutations.

RepoPilot analysis is now explicit. Entering Changes does not spawn analysis work in the background.

## Work visual hierarchy

The default Work surface answers three questions:

```text
Where am I?
What happens in this phase?
What is the next allowed action?
```

The former four-column phase explanation (`Input / RepoDesk does / Result / Then`) has been replaced by one Current Step block.

Execution still exposes executor/model/workspace/token/cost facts before launch, but routed steps and packet explanation live under disclosure.

Advanced orchestration remains available without competing with the current Work Item action.

## Changes visual hierarchy

Default:

```text
branch / dirty state / actions
commit gate

Files | Selected diff
```

On demand:

```text
Evidence → provenance, scope, review, verification, override
Findings → RepoPilot trend and file findings
```

Normal compliant scope is intentionally silent per file. Exceptional states remain visible:

- out of scope;
- protected;
- ungoverned;
- unattributed;
- blocking finding.

## Runtime measurements v0

`runtimeMetrics.ts` keeps aggregate metadata only:

- command;
- calls;
- errors;
- total duration;
- max duration;
- last duration.

No command arguments, source, prompt, result body or secret-shaped payload is retained by this metric store.

The store tracks at most 96 command keys and folds additional command names into `__other__`.

### Important limitation

This first measurement path covers calls using the shared `callCommand` transport.

Several typed APIs still call Tauri `invoke` directly, including parts of orchestration and engineering. The Debug UI therefore calls the data **Instrumented IPC cost**, not total application IPC cost.

A later transport-consolidation slice should migrate typed APIs to one measured transport without losing their typed wrappers.

## Non-goals

This slice does not:

- change Work Item, ChangeSet, verification or commit receipt semantics;
- weaken required approvals;
- hide commit blockers;
- add synthetic benchmark claims;
- add Monaco/CodeMirror;
- add LSP;
- change PTY lifetime;
- convert every legacy Tauri invocation to the measured transport;
- redesign every secondary screen.

## Local verification

```bash
git fetch origin
git checkout perf/rd2-performance-ui-density-v1
git pull

pnpm --dir apps/desktop build
cargo check -p repodesk-desktop
```

## Manual smoke

### Work

1. Leave Work open in a stable phase and verify there is no visible four-second refresh cycle.
2. Build context and confirm the phase/evidence refreshes after the action.
3. Launch an agent run and confirm pending state remains visible until the command completes.
4. Accept/reject a run and confirm Git + Work + Changes data refreshes.
5. Verify and finish using the existing receipt-bound gates.

### Changes

1. Open Changes with modified files. RepoPilot must not run automatically.
2. Confirm compliant files do not all show redundant `In scope` pills.
3. Open Evidence and confirm full governance is still available.
4. Click Analyze and confirm Findings opens on demand.
5. Double-click a file and confirm it opens in Code.

### Runtime

1. Use Work/Code/Changes for several actions.
2. Open Debug → Runtime.
3. Confirm instrumented command counts and durations are visible.
4. Reset the metric window and confirm totals return to zero.
5. Confirm the Runtime section never contains command payloads or source content.
