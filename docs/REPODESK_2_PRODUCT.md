# RepoDesk 2.0 — Product Foundation

## Status

Draft product contract for the RepoDesk 2.0 direction.

This document intentionally changes RepoDesk from an AI operations dashboard into a local-first engineering environment.

## Product thesis

RepoDesk is a **local-first engineering workspace for planning, executing, reviewing, verifying, and learning from software changes made by humans and coding agents**.

RepoDesk is not primarily an AI provider manager and not a second generic AI control plane. AI is one class of worker inside the engineering system.

The central object is not a provider, model, prompt, file, or chat session.

The central object is a **Work Item**.

A Work Item connects:

- goal;
- repository scope;
- acceptance criteria;
- relevant code and tests;
- bounded context;
- human and agent execution;
- isolated workspaces;
- changesets;
- verification evidence;
- review decisions;
- commits;
- project knowledge;
- engineering telemetry.

The primary lifecycle is:

```text
Scope -> Prepare -> Execute -> Review -> Verify -> Finish -> Learn
```

## Product boundary with SubRadar

SubRadar owns generic AI control-plane concerns:

- normalized AI requests;
- provider/model policy;
- AI budget and spend;
- privacy policy;
- generic AI usage ledger;
- AI execution profiles;
- user-level AI preferences and memory;
- provider/model quality signals.

RepoDesk owns software-engineering concerns:

- repositories and projects;
- work items;
- code and symbols;
- engineering context;
- project knowledge;
- coding-agent execution;
- worktrees and isolation;
- Git changesets;
- verification and diagnostics;
- engineering rules;
- review and commit readiness;
- engineering workflow intelligence.

The intended integration is:

```text
RepoDesk determines WHAT engineering work is required.
SubRadar may determine HOW a generic inference request should execute.
RepoDesk remains responsible for workspace-changing execution and verification.
```

RepoDesk may continue to contain provider/runtime adapters during migration, but they are infrastructure rather than the product identity.

## Product pillars

### 1. Work

Every meaningful development action belongs to a Work Item.

A Work Item should contain:

- title and goal;
- scope;
- constraints;
- acceptance criteria;
- execution plan;
- relevant files and symbols;
- context pack;
- workers;
- changesets;
- checks;
- decisions;
- evidence;
- final outcome.

RepoDesk should make it hard to begin broad, ambiguous agent work without first defining scope.

### 2. Code

RepoDesk should become an engineering workspace rather than a dashboard around another editor.

The Code surface should evolve toward:

- project tree;
- file tabs;
- Monaco-based viewing and editing;
- search;
- changed-line indicators;
- symbol navigation;
- diagnostics;
- related tests;
- project knowledge attached to code;
- later, LSP-backed navigation and refactoring.

RepoDesk does not need to compete with VS Code extension breadth. The editor exists to keep the engineering workflow in one place.

### 3. Changes

A changeset is a first-class object.

RepoDesk must be able to answer:

- who produced these changes;
- for which Work Item;
- in which workspace/worktree;
- what files changed;
- what the diff is;
- whether scope was respected;
- what verification ran;
- whether the human accepted or rejected it;
- whether it is ready to commit.

Coding-agent output must remain attributable and reviewable.

### 4. Verification

RepoDesk should unify engineering signals into one Problems/Evidence model.

Potential problem sources:

- compiler;
- tests;
- linter;
- formatter;
- security scanners;
- architecture checks;
- Git state;
- project rules;
- agent-scope violations;
- failed verification receipts.

Checks should execute locally when possible. AI should receive bounded failure evidence rather than complete noisy logs.

### 5. Engineering Knowledge

Generic memory becomes **Engineering Knowledge**.

Knowledge types include:

- architecture decisions;
- repository conventions;
- known pitfalls;
- commands;
- subsystem descriptions;
- invariants;
- testing expectations;
- dependency constraints;
- glossary entries;
- lessons learned from accepted/rejected changes.

Knowledge must carry provenance and scope.

Suggested metadata:

```text
id
project
kind
content
source_type
source_id
confidence
scope
created_at
last_used_at
usage_count
status
```

Durable knowledge remains review-first.

### 6. Engineering Intelligence

RepoDesk should measure the engineering process itself.

This is not a vanity analytics dashboard. Metrics exist to answer:

> Are we solving the task with less context, fewer agents, fewer retries, safer changes, and more reusable understanding?

The system should retain raw evidence and derive explainable metrics from it.

## Engineering Intelligence model

### A. Task complexity

RepoDesk should estimate task complexity before execution and update the estimate after execution.

Signals may include:

- scoped file count;
- subsystem count;
- dependency depth;
- number of languages involved;
- public API impact;
- migration/schema involvement;
- concurrency/security involvement;
- expected test surface;
- number of required workers;
- historical failure rate for similar work.

Initial categories:

```text
trivial
small
medium
large
high-risk
```

The estimate must be heuristic and explainable, not presented as mathematical truth.

### B. Algorithmic profile

RepoDesk should understand algorithmic characteristics of code where feasible.

It should eventually record per function/module:

- loops and nesting depth;
- recursion;
- dominant collection operations;
- sorting/searching patterns;
- allocations in hot loops;
- repeated scans;
- likely time-complexity class;
- likely auxiliary-space class;
- confidence and evidence.

Example result:

```text
Function: resolve_dependencies
Estimated time: O(V + E)
Estimated space: O(V)
Confidence: high
Evidence:
- one traversal over nodes
- one traversal over adjacency edges
- visited hash set
```

For ambiguous code, RepoDesk must say `unknown` or provide multiple possible bounds rather than invent certainty.

This is an engineering aid, not a formal complexity prover.

### C. Context Compactness

Measure whether workers receive only useful context.

Candidate metrics:

```text
context_tokens
selected_files
used_files
changed_files
context_to_change_ratio
context_reuse_ratio
irrelevant_context_ratio
```

A useful first metric:

```text
Context Compactness = useful_context_units / total_context_units
```

`useful_context_units` must be evidence-backed, for example files referenced by the worker result, changed files, files used in verification, or files explicitly confirmed as relevant.

Do not optimize blindly for the smallest context. The target is sufficient context with minimal noise.

### D. Agent fan-out

Track how many workers were used to complete one Work Item.

```text
worker_count
parallel_wave_count
handoff_count
agent_to_agent_handoffs
human_handoffs
```

High fan-out is not automatically bad. RepoDesk should distinguish useful decomposition from wasteful duplication.

### E. Agent redundancy

Estimate duplicated work across agents.

Signals:

- same files read by multiple agents;
- same question answered repeatedly;
- overlapping changesets;
- duplicate context packs;
- repeated failed attempts with unchanged input;
- multiple reviews reaching the same conclusion.

Candidate metric:

```text
Redundancy Ratio = duplicated_work_units / total_work_units
```

The exact scoring method can evolve. Evidence must remain inspectable.

### F. Retry and correction cost

Track:

- execution attempts;
- failed attempts;
- rejected changesets;
- verification failures;
- rework after review;
- context rebuilds;
- agent reroutes.

This lets RepoDesk distinguish an apparently cheap first attempt from an expensive workflow that required five corrections.

### G. Knowledge reuse

Measure whether durable project knowledge prevents re-discovery.

Signals:

- knowledge entries injected into context;
- entries referenced by plans;
- entries associated with successful execution;
- stale entries;
- conflicting entries;
- knowledge created from accepted work;
- repeated questions that should already be answered by knowledge.

Candidate metrics:

```text
knowledge_reuse_rate
stale_knowledge_rate
new_knowledge_per_work_item
rediscovery_rate
```

### H. Evidence density

Measure how much decision-relevant evidence exists relative to raw output.

Examples:

- summarized failing tests instead of complete logs;
- bounded diffs instead of full repository dumps;
- explicit source references for project rules;
- verification receipts bound to a tree state.

Candidate metric:

```text
Evidence Density = decision_relevant_evidence / captured_output
```

### I. Human override signal

Human decisions are valuable learning evidence.

Track:

- accepted agent changes;
- rejected agent changes;
- partially accepted changes;
- plan edits;
- context edits;
- route overrides;
- manual fixes after agent execution;
- knowledge proposals accepted/rejected.

RepoDesk should learn from repeated human corrections while keeping automatic adaptation bounded and explainable.

### J. Engineering efficiency

Do not create one magic score initially.

Store independent dimensions first:

- context compactness;
- retry rate;
- verification success;
- scope adherence;
- knowledge reuse;
- agent redundancy;
- elapsed time;
- cost/tokens when available;
- accepted-change ratio.

A composite score can be considered later only when its meaning is clear.

## Worker model

RepoDesk should treat AI as one worker type among several.

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

A Work Item may use several workers.

Each execution should record:

```text
worker_type
worker_id
capability
workspace
input_context_id
started_at
finished_at
result
changed_files
evidence
cost
tokens
approval
```

## Target information architecture

Primary surfaces:

### Work

Active Work Item, lifecycle, plan, approvals, context, execution and next action.

### Code

Repository tree, editor, search, symbols, diagnostics, code-related knowledge.

### Changes

Git state, changesets, diffs, worktrees, review, staging and commit readiness.

### Runs

Worker executions, checks, receipts, history, failures and evidence.

### Projects

Project registry, engineering rules, project knowledge, commands, playbooks and repository configuration.

Secondary/system surfaces:

- Settings;
- Debug;
- platform/plugin management.

## IDE-like shell

Long-term shell structure:

```text
┌──────────────────────────────────────────────────────────────────┐
│ project | branch | work item | workspace state | command palette │
├─────┬───────────────┬───────────────────────────┬────────────────┤
│rail │ project panel │ central workspace         │ inspector      │
│     │               │                           │                │
├─────┴───────────────┴───────────────────────────┴────────────────┤
│ terminal | problems | checks | agent output | logs               │
└──────────────────────────────────────────────────────────────────┘
```

The shell should be optimized for one active engineering task, not a collection of dashboards.

## Problems model

A future normalized problem record should look approximately like:

```text
Problem
- id
- project
- work_item
- source
- severity
- category
- message
- file
- line/column
- evidence
- suggested_action
- created_at
- resolved_at
```

This allows tests, compiler errors, architecture findings and agent violations to appear in one place without pretending they are identical internally.

## Project rules

RepoDesk should support explicit engineering policy.

Examples:

```yaml
architecture:
  domain_cannot_import_ui: true
  max_module_depth: 5

execution:
  require_isolated_worktree_for_agents: true
  max_agent_changed_files: 12

git:
  require_review_before_stage: true
  require_verification_before_commit: true

verification:
  required:
    - cargo fmt --all -- --check
    - cargo clippy --workspace --all-targets --all-features -- -D warnings
    - cargo test --workspace

knowledge:
  architecture_changes_require_decision: true
```

Rules should be deterministic where practical and explainable when blocking an action.

## Non-goals

RepoDesk 2.0 is not trying to become:

- a generic chat application;
- a full replacement for every VS Code extension;
- a cloud collaboration suite in the first phase;
- an autonomous system allowed to commit/push/merge without human policy;
- a second provider-spend dashboard competing with SubRadar;
- a formal theorem prover for Big-O complexity;
- an unrestricted agent shell.

## Success condition

RepoDesk 2.0 succeeds when a developer can perform a meaningful task primarily from RepoDesk and later answer:

1. What was the task and scope?
2. What code and knowledge were relevant?
3. Which human/agent/tool performed each step?
4. What context did each worker receive?
5. What changed?
6. Did the changes respect project rules?
7. What verification proves the current changeset?
8. What was accepted or rejected and why?
9. What did the project learn?
10. Was the workflow compact or wasteful?
11. Did algorithmic complexity improve, regress, or remain unknown?

That is the core difference between RepoDesk and a generic AI controller.