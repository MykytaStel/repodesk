# ADR-0001 — RepoDesk 2.0 Product Boundary

- Status: Proposed
- Date: 2026-08-07

## Context

RepoDesk evolved from a local AI operations cockpit into a system that already contains repository management, tasks, bounded context, coding-agent execution, isolated worktrees, changeset review, verification, project memory, orchestration, provider routing, token/cost tracking, and outcome learning.

At the same time, SubRadar is evolving into a generic local-first AI control plane responsible for AI request policy, providers, profiles, budgets, privacy, request ledgers, and user-approved AI memory.

Without a product boundary, both applications would own similar provider/model/routing/memory concepts and differ mostly by RepoDesk having Git integration.

That is not a durable product separation.

## Decision

RepoDesk 2.0 becomes a **local-first engineering environment for controlled software change**.

The primary domain object is a `WorkItem`.

RepoDesk owns the lifecycle:

```text
WorkItem
  -> Scope
  -> Prepare context
  -> Execute workers
  -> Produce ChangeSet
  -> Review
  -> Verify
  -> Finish/commit
  -> Learn
```

RepoDesk treats AI as a worker/runtime dependency rather than as the center of the product.

SubRadar owns generic AI request governance.

## Ownership boundary

### RepoDesk owns

- repository/project state;
- work items;
- code/files/symbols;
- engineering context packs;
- engineering knowledge;
- coding-agent execution;
- isolated worktrees;
- changesets and diffs;
- Git review/staging/commit readiness;
- checks and verification;
- engineering problems/diagnostics;
- project rules;
- engineering workflow telemetry;
- algorithmic profile/complexity evidence;
- human acceptance/rejection evidence.

### SubRadar owns

- generic inference request normalization;
- provider/model policy;
- provider credentials and generic availability policy over time;
- AI budget/spend governance;
- generic AI usage ledger;
- privacy policy for inference;
- execution profiles;
- generic user-level AI memory/preferences;
- provider/model performance signals.

## Integration boundary

RepoDesk may issue an inference request with engineering metadata such as:

```text
project
work_item
purpose
complexity
privacy requirement
web/tool requirement
context size
output budget
```

SubRadar may return an execution decision and inference result.

RepoDesk remains authoritative for:

- whether a worker may modify a repository;
- which worktree/workspace is used;
- what files are in scope;
- whether a changeset is accepted;
- which checks are required;
- whether a change is ready to commit.

A generic AI runtime must never bypass RepoDesk's workspace, changeset, review, or verification rules.

## Worker abstraction

The engineering runtime should move toward a common worker vocabulary:

```text
Worker
├── Human
├── CodingAgent
├── CompletionModel
├── CheckRunner
├── Script
├── CI
└── ExternalTool
```

This prevents provider/model concepts from leaking into every engineering domain object.

## Memory boundary

RepoDesk `Memory` evolves into `EngineeringKnowledge`.

Examples:

- architectural invariants;
- repository conventions;
- subsystem descriptions;
- known pitfalls;
- required commands;
- lessons learned from accepted/rejected work.

Generic preferences such as preferred model/provider, private profile, or global reasoning preference belong to SubRadar.

## Learning boundary

Provider/model success-rate learning should eventually live in SubRadar.

RepoDesk learning should focus on engineering outcomes:

- scope adherence;
- accepted/rejected changes;
- verification success;
- retries and rework;
- useful context;
- knowledge reuse;
- algorithmic regressions/improvements;
- repeated failure patterns;
- repository-specific rules.

## UI consequence

RepoDesk should no longer be designed as a collection of AI dashboards.

The target shell is IDE-like:

- activity rail;
- project/work-item explorer;
- central workspace/editor;
- contextual inspector;
- bottom panel for terminal/problems/checks/agent output.

Primary surfaces become:

- Work;
- Code;
- Changes;
- Runs;
- Projects.

Models, tokens, generic routing, and provider spend become secondary infrastructure during migration and may later delegate to SubRadar.

## Security consequence

The existing fail-closed principles remain:

- isolated worktrees for coding-agent writes;
- explicit approval for gated execution;
- bounded context;
- secret scanning;
- no unrestricted shell exposed as an agent capability;
- human review before durable changes;
- verification bound to the reviewed tree/change state.

## Consequences

### Positive

- RepoDesk has a durable product identity independent of current AI vendors.
- SubRadar and RepoDesk become complementary.
- Existing worktree/review/check infrastructure becomes central rather than secondary.
- The application can evolve into an AI-native IDE without becoming a generic editor clone.
- Engineering telemetry can optimize the complete workflow rather than only provider cost.

### Negative

- Existing UI and terminology require migration.
- Some provider/token functionality becomes transitional or secondary.
- `Memory`, `Outcomes`, `Orchestrate`, and several tabs require consolidation.
- New domain types will coexist with legacy types during migration.

## Migration rule

Do not perform a big-bang rewrite.

Each slice must preserve current working paths and migrate one boundary at a time behind typed core APIs and tests.

## Acceptance criteria for this ADR

This ADR is considered adopted when:

- README/product docs use the engineering-environment identity;
- new feature proposals identify whether they belong to RepoDesk or SubRadar;
- new RepoDesk AI features are justified by an engineering workflow need;
- the RepoDesk 2 migration roadmap is the default direction for new product work.