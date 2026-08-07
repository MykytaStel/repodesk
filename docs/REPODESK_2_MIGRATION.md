# RepoDesk 2.0 — Migration Roadmap

This roadmap moves the existing RepoDesk product toward an IDE-like engineering environment without a big-bang rewrite.

## Migration principles

1. Keep current workflows operational while new surfaces are introduced.
2. Prefer typed domain extraction over frontend-only rearrangement.
3. Move one responsibility at a time.
4. Preserve evidence, safety, and review guarantees.
5. Do not delete provider/runtime code until the replacement boundary is real.
6. Build new product surfaces around `WorkItem`, `ChangeSet`, `Verification`, and `EngineeringKnowledge`.
7. Add metrics only when their evidence can be inspected.

## Target primary navigation

```text
Work
Code
Changes
Runs
Projects
```

Secondary:

```text
Settings
Debug
Platform
```

## Existing feature migration map

| Existing area | RepoDesk 2 destination | Direction |
|---|---|---|
| Work | Work | Keep and strengthen as primary surface |
| Orchestrate | Work / Runs | Absorb execution controls into Work; run details into Runs |
| Dashboard | Work | Remove as separate destination; show task-level overview inline |
| Code | Code | Expand into real code workspace |
| Git | Changes | Absorb completely |
| Changes | Changes | Keep and expand |
| History | Runs | Rename/reframe as execution and evidence history |
| Outcomes | Runs / Engineering Intelligence | Keep raw evidence; stop centering provider score |
| Audit | Runs / Debug | Human-readable evidence in Runs; deep audit in advanced mode |
| Memory | Projects / Engineering Knowledge | Rename and change semantics |
| Playbooks | Projects | Repository-scoped engineering workflows |
| Tokens | Runs / secondary telemetry | Per-run/task telemetry, not primary navigation |
| Models | Settings / SubRadar bridge | Secondary runtime infrastructure |
| Models & Cost | Settings / Runs / SubRadar bridge | Remove as primary surface |
| Routing | runtime infrastructure | Keep internal until SubRadar bridge replaces generic inference routing |
| System Registry | Platform | Advanced capabilities/plugins/MCP surface |
| Debug | Debug | Keep advanced |
| Settings | Settings / Projects | Split project configuration from application/runtime configuration |

## RD2-01 — Product boundary and vocabulary

Goal: establish the new product contract before code restructuring.

Deliverables:

- `REPODESK_2_PRODUCT.md`;
- product-boundary ADR;
- migration map;
- README identity update;
- terminology glossary.

No behavior changes.

## RD2-02 — Core domain vocabulary

Introduce typed domain concepts without removing legacy APIs.

Target types:

```rust
struct WorkItemId(...);
struct WorkItem { ... }
struct ExecutionId(...);
struct WorkerRef { ... }
struct ChangeSetId(...);
struct ChangeSet { ... }
struct VerificationId(...);
struct VerificationReceipt { ... }
struct EvidenceRef { ... }
struct EngineeringKnowledgeId(...);
```

Initial implementation should adapt existing task/orchestrator/worktree/check structures rather than duplicating behavior.

Acceptance:

- new types compile and serialize;
- conversion from current task/run models exists;
- tests cover identity and lifecycle invariants;
- no frontend migration required yet.

## RD2-03 — Engineering event ledger

Before building intelligence, normalize observable workflow events.

Suggested event categories:

```text
work_item_created
scope_changed
context_built
context_edited
execution_started
execution_finished
worker_handoff
changeset_created
changeset_reviewed
verification_started
verification_finished
commit_created
knowledge_proposed
knowledge_accepted
knowledge_rejected
human_override
```

Each event should include stable IDs and optional evidence references.

This ledger becomes the factual substrate for later metrics.

## RD2-04 — Engineering Intelligence v0

Implement metrics that can be derived safely from existing evidence.

First metrics:

- execution attempt count;
- worker count;
- agent count;
- handoff count;
- changed file count;
- context token count;
- verification pass/fail count;
- accepted/rejected changeset count;
- total run cost/tokens if available;
- knowledge entries injected/reused;
- scope violations;
- elapsed time.

Do not add a composite score.

Expose as typed core report + CLI JSON first.

## RD2-05 — Context Compactness v0

Add evidence-backed context telemetry.

Track:

```text
included_files
context_tokens
changed_files
referenced_files
verification_related_files
knowledge_entries
```

Derive conservative ratios such as:

```text
change_coverage = changed_files_present_in_context / changed_files
context_to_change_ratio = context_tokens / max(changed_lines, 1)
```

Do not label unreferenced context as useless automatically.

UI should explain exactly how each value was derived.

## RD2-06 — Algorithmic Profile v0

Create a deterministic static-analysis foundation for obvious complexity signals.

Start narrow:

- Rust;
- JavaScript/TypeScript later.

Rust v0 signals:

- function-level loop count;
- maximum loop nesting;
- explicit recursion detection where resolvable;
- collection operations inside loops;
- sort calls;
- repeated iterator scans;
- obvious nested scans over the same input;
- allocation-like calls in loops;
- function LOC/cyclomatic-style structural proxy.

Output:

```text
AlgorithmicProfile {
  symbol,
  time_complexity_hint,
  space_complexity_hint,
  confidence,
  evidence[],
  warnings[]
}
```

Initial classes:

```text
O(1)
O(log n)
O(n)
O(n log n)
O(n^2)
O(n^k)
unknown
```

The analyzer must prefer `unknown` to unsupported certainty.

Later slices can integrate AST parsers/tree-sitter/rust-analyzer data.

## RD2-07 — Workspace shell

Replace dashboard-style navigation with an IDE-like shell.

Structure:

```text
Top context bar
Activity rail
Contextual side panel
Central workspace
Inspector
Bottom panel
```

Primary destinations become Work, Code, Changes, Runs, Projects.

Legacy routes remain deep-linkable during migration.

## RD2-08 — Work consolidation

Absorb orchestration into the active Work Item.

The Work surface should show:

- lifecycle phase;
- scope;
- context status;
- worker plan;
- approvals;
- execution preview;
- current/last changeset;
- verification status;
- next safe action.

Advanced orchestration remains a disclosure panel rather than a top-level destination.

## RD2-09 — Changes workspace

Merge Git, diffs, worktrees, and review into one surface.

Views:

```text
Working Tree
Agent Worktrees
ChangeSets
Staged
Conflicts
Commit Readiness
```

Every changeset should carry origin metadata.

## RD2-10 — Runs and Evidence

Consolidate History, Outcomes, and user-facing Audit.

A run detail should show:

- Work Item;
- worker;
- context pack;
- timings;
- tokens/cost where available;
- changed files;
- verification;
- review outcome;
- receipts;
- knowledge proposals;
- engineering-intelligence metrics.

Provider-performance learning becomes secondary and eventually migrates to SubRadar.

## RD2-11 — Engineering Knowledge

Rename/reframe project memory.

Knowledge categories:

```text
architecture
convention
invariant
command
pitfall
subsystem
decision
lesson
glossary
```

Add provenance, confidence, scope, status, and reuse counters.

Context assembly should record which knowledge entries were injected.

## RD2-12 — Code workspace v0

Add an actual editing workspace without attempting full IDE parity.

First slice:

- repository tree;
- open-file tabs;
- Monaco editor;
- read/write/save;
- find in file;
- syntax highlighting;
- dirty-state indicator;
- diff markers;
- open changed file from Changes.

RepoDesk security/path guards remain authoritative for file access.

## RD2-13 — Problems model

Normalize diagnostics from multiple sources.

Sources:

- compiler;
- formatter;
- linter;
- tests;
- architecture/security analysis;
- project rules;
- agent scope violations;
- Git/worktree failures.

Expose in bottom panel with filtering and navigation to file/location/evidence.

## RD2-14 — Task Runner and Terminal

Two distinct concepts:

### Task Runner

RepoDesk-controlled configured commands with receipts.

Examples: build, test, lint, format, dev, audit.

### Terminal

Interactive human shell.

The existence of a human terminal must not grant unrestricted shell access to coding agents.

## RD2-15 — LSP foundation

Start with Rust.

Integrate language-server lifecycle behind a generic protocol boundary.

First capabilities:

- diagnostics;
- go to definition;
- hover;
- find references.

Then add TypeScript.

## RD2-16 — Repo Intelligence

Build repository understanding from multiple evidence sources:

- file graph;
- imports/dependencies;
- symbols/references;
- tests;
- Git history;
- Work Item history;
- Engineering Knowledge;
- Algorithmic Profile.

Target user questions:

```text
What depends on this symbol?
Which tests are closest to this change?
Which files usually change together?
What is risky about changing this subsystem?
Did this changeset worsen algorithmic complexity?
```

## RD2-17 — Agent Coordination Intelligence

Use the engineering event ledger to detect workflow waste.

Signals:

- excessive agent fan-out;
- repeated identical context;
- overlapping agent work;
- repeated retries without changed evidence;
- unnecessary provider escalation;
- duplicated reviews;
- context rebuilt without meaningful change.

Recommendations must be explainable, e.g.:

```text
3 agents received >85% identical context and produced no independent changes.
Suggestion: use one implementation worker and one reviewer.
```

## RD2-18 — SubRadar runtime bridge

Introduce a generic inference runtime boundary.

RepoDesk request:

```text
purpose
project/work-item attribution
complexity
privacy requirement
context size
web/tools requirements
output budget
```

SubRadar returns execution/runtime information.

Coding-agent workspace execution remains in RepoDesk.

Migration should allow direct provider adapters as fallback until the bridge is mature.

## RD2-19 — Plugin/MCP platform

Expose controlled extension points for:

- repository readers;
- diagnostics;
- task runners;
- knowledge sources;
- issue trackers;
- CI;
- local services.

Extensions must declare capabilities and remain inside RepoDesk security policy.

## Recommended immediate PR sequence

### PR 1 — Product foundation

Docs only:

- product contract;
- ADR;
- migration roadmap;
- README update.

### PR 2 — Engineering event/domain foundation

- `WorkItemId`/`ExecutionId`/`ChangeSetId` typed IDs;
- engineering event enum;
- SQLite migration;
- unit tests;
- no UI changes.

### PR 3 — Intelligence v0

- derive factual metrics from current orchestrator/check/change evidence;
- CLI report command;
- fixtures/tests.

### PR 4 — Workspace shell skeleton

- new shell layout;
- route legacy features through the new rail;
- no behavior removal.

### PR 5 — Work/Changes consolidation

- remove Orchestrate from primary navigation;
- absorb Git/Code diff paths into Changes;
- preserve deep links.

Only after these foundations should Monaco, terminal, LSP, and deeper repository intelligence be introduced.