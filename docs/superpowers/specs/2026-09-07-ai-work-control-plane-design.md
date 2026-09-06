# RepoDesk AI Work Control Plane

## Status

Proposed design, based on the product direction approved in conversation on
2026-09-07. This document is a product and architecture contract, not an
implementation plan. No runtime behavior is changed by this document.

## Executive decision

RepoDesk should become a local-first **AI Engineering Control Plane** for
developers who use coding agents as part of real software work.

The product is not:

- another coding agent;
- a generic token dashboard;
- a CI replacement;
- a predictive test selector detached from the development workflow;
- a productivity surveillance system.

The product promise is:

> **Know what this run will cost, what it proved, and when to stop.**

RepoDesk helps a developer choose the next economical, evidence-producing
action: continue an agent, reduce or rebuild context, switch workers, run a
targeted check, run the full verification set, pause for human review, or
stop with a clearly recorded partial result.

The central optimization target is not minimum token spend. It is:

> **minimum total cost of a trusted, accepted software change.**

That cost includes model usage, test execution, retries, correction work,
human review, waiting time, and the cost of discovering that a skipped check
was needed.

## Why this direction is needed

The current RepoDesk foundation already contains Work Items, bounded context,
agent execution, worktrees, changesets, verification receipts, usage ledgers,
and Engineering Intelligence. The problem is product convergence: these
capabilities can look like separate Models, Tokens, Runs, Audit, and
Dashboard destinations instead of one decision loop.

The new direction makes the decision loop the product surface:

```text
Work Item
  -> agent or human run
  -> cost, context, and change evidence
  -> verification decision
  -> independent proof
  -> accepted or rejected change
  -> learning for the next run
```

## Market validation signals

This is directional validation, not a statistically representative market
study. Reddit and forum posts are anecdotal and include low-signal or
automated content, so they are treated as problem signals and triangulated
with Hacker News, GitHub Discussions, project documentation, and existing
tools.

Repeated signals:

| Observed problem | What users appear to want | RepoDesk implication |
| --- | --- | --- |
| Agentic usage is unpredictable and small tasks can consume a large allowance | Pre-run estimates, visible budget, and predictable fallback behavior | Add a preflight estimate and explicit budget/stop policy |
| Agents retry or loop without recognizing that they are wasting money | Hard caps, iteration limits, loop detection, and graceful partial completion | Make runaway protection a first-class runtime control |
| Aggregate token totals do not explain where spend went | Attribution by work item, repository, model, worker, retry, and cache class | Bind every usage record to an engineering run and outcome |
| An agent saying “done” or “tests pass” is not trusted | Independent command receipts, actual exit status, test count, and tree identity | Never treat a worker summary as verification proof |
| Full test suites are slow and their output is expensive to feed back to an agent | Targeted checks, failure grouping, compact diagnostics, and explicit deferred checks | Build Adaptive Verification around evidence and cost |
| Users want cheaper models for routine work but stronger models for risky work | Repo- and task-specific routing based on observed outcomes | Learn model fitness from accepted changes, not generic leaderboards |
| Developers want to understand whether AI actually saves time | Cost per accepted change, correction cost, trust/verification time, and rework | Replace vanity analytics with decision-oriented Engineering Intelligence |

Representative sources include:

- [GitHub Discussion 197872](https://github.com/orgs/community/discussions/197872)
  on unpredictable credit consumption during agentic work;
- [Ask HN: How are you keeping AI coding agents from burning money?](https://news.ycombinator.com/item?id=47559293)
  on per-task attribution, retry limits, and model fitness by repository/task;
- [Reddit discussion on trusted finished work](https://www.reddit.com/r/AI_Agents/comments/1u37ntx/are_coding_agents_getting_expensive_or_are_we/)
  on outcome cost being more meaningful than token cost;
- [Reddit discussion on agents claiming tests pass](https://www.reddit.com/r/ClaudeAI/comments/1w0lv7b/do_your_agents_claim_tests_pass_without_actually/)
  on the need for receipts rather than self-reported completion;
- [Hacker News discussion on automated verification](https://news.ycombinator.com/item?id=47397367)
  on intent alignment and evidence beyond test pass rate;
- [OpenClaw loop-budget proposal](https://github.com/openclaw/openclaw/issues/107423)
  on token budgets, iteration caps, escalation, and report-only mode;
- [Launchable predictive test selection documentation](https://help.launchableinc.com/features/predictive-test-selection/how-launchable-selects-tests/)
  as evidence that test selection is already a distinct category, but is
  primarily optimized around CI history rather than an AI work session;
- [TokenAnalytics documentation](https://www.tokenanalytics.app/docs/)
  as evidence that local multi-agent token and cost dashboards already exist.

### What is and is not validated

Validated enough to pursue:

- cost anxiety and runaway agent loops are real problems for heavy users;
- users want per-task and per-run attribution rather than only monthly totals;
- independent verification is a trust problem, not a cosmetic feature;
- the useful unit is closer to trusted finished work than raw tokens;
- a decision layer connecting code changes, agent traces, checks, and outcomes
  is a plausible product gap.

Not yet validated:

- willingness to pay for a standalone desktop control plane;
- whether casual AI coding users will adopt a separate application;
- the correct default policies for every language and repository;
- whether teams or individual power users should be the first commercial focus.

The first target should therefore be solo power developers and small teams
using multiple or metered coding agents. Product-market validation must happen
with a small design-partner group before broad feature expansion.

## Product boundary

### RepoPilot

RepoPilot remains the deterministic repository proof provider:

- structural change analysis;
- contract and behavior signals;
- blast radius;
- security and algorithmic findings;
- proof obligations;
- conservative verdicts;
- CLI, CI, and MCP access.

RepoPilot answers:

> **What changed, what is affected, and what does the repository evidence prove
> or fail to prove?**

### RepoDesk

RepoDesk owns the dynamic engineering workflow:

- Work Items and acceptance criteria;
- context and worker selection;
- budgets and stop conditions;
- execution and retry control;
- check selection and verification scheduling;
- evidence and human decisions;
- cost-to-outcome analytics;
- project-specific learning.

RepoDesk answers:

> **What should happen next, is the next action worth its cost, and can this
> change be accepted?**

RepoDesk consumes RepoPilot's `ChangeProof`; it must not reimplement or
contradict RepoPilot verdict logic.

### Other agents

Codex, Claude Code, Hermes, local models, scripts, CI, and humans are workers.
They may be used through adapters that produce normalized execution events.
RepoDesk is not required to own the worker's chat, memory, skills, or provider
marketplace.

## Core product concept: Adaptive Verification

Adaptive Verification is a policy and evidence loop that recommends or enforces
the least expensive next verification action that can reduce meaningful
uncertainty.

### Inputs

- Work Item goal and acceptance criteria;
- current Git tree and diff identity;
- changed paths, symbols, and repository rules;
- RepoPilot `ChangeProof` and proof obligations;
- project check catalog;
- historical check duration and failure behavior;
- current context and worker state;
- run budget, time budget, and user policy;
- prior attempts and unresolved verification debt.

### Outputs

Every decision must produce one of these explicit actions:

```text
run_now
run_targeted
defer_with_debt
ask_for_approval
pause_and_review
stop_with_partial_result
```

Every output includes:

- the decision and its policy version;
- observed facts and evidence references;
- estimated token, money, and wall-clock cost;
- selected checks and skipped checks;
- reason for the recommendation;
- risk and uncertainty labels;
- what remains unproven;
- whether a human override is required or was used.

The first implementation should be recommendation-first. Automatic skipping or
automatic routing is allowed only when an explicit project policy enables it.

### Verification policy principles

1. Deterministic facts precede model judgment.
2. Critical project checks cannot be silently skipped.
3. Unknown is a valid result; lack of evidence is not proof of safety.
4. A skipped check creates visible verification debt.
5. A check receipt is bound to an exact tree/worktree identity.
6. The implementation agent cannot be the sole authority that declares its
   own work complete.
7. A policy decision is reversible and inspectable.
8. Token savings must not be reported without showing the quality evidence
   that made the saving acceptable.

## Decision Receipt

`DecisionReceipt` is the canonical RepoDesk artifact for an adaptive decision.
It is distinct from RepoPilot's `ChangeProof`.

Minimum shape:

```text
decision_id
work_item_id
execution_id
project_id
tree_identity
changeset_identity
decision_kind
policy_version
observed_facts[]
evidence_refs[]
selected_actions[]
skipped_actions[]
estimated_cost
actual_cost_after_completion
risk_label
uncertainty_label
verification_debt[]
human_override
outcome
created_at
```

Examples:

- “Run targeted Rust tests because the changed symbol has three direct
  consumers and the full suite costs 14 minutes.”
- “Do not start another agent retry because the previous context and tree are
  unchanged and the same failure repeated twice.”
- “Pause before commit because the browser acceptance criterion has no real
  browser receipt.”

## Runtime and data architecture

### Event substrate

Extend the existing append-only Engineering Intelligence event ledger rather
than introducing a parallel analytics database.

Events should remain factual and replayable. Derived metrics and
recommendations must reference the events that produced them.

Additional event types:

```text
preflight_estimated
budget_policy_applied
budget_threshold_reached
loop_pattern_detected
worker_paused
worker_stopped
verification_recommended
verification_selected
verification_deferred
verification_receipt_created
failure_grouped
decision_overridden
decision_outcome_recorded
```

Normalized runtime fields should include:

```text
provider
model
worker
work_item
execution
attempt
step
input_tokens
output_tokens
cached_input_tokens
reasoning_tokens
estimated_cost
actual_cost_when_available
tool_call_count
duration
retry_index
context_identity
tree_identity
outcome
```

Raw prompts and full model responses are not required for the first analytics
version. Local metadata and bounded evidence should be sufficient for most
decisions and protects user privacy.

### Control layers

```text
Worker adapters
  -> normalized run ledger
  -> policy engine
  -> execution/check controller
  -> Decision Receipts + verification receipts
  -> outcome analytics and project learning
```

The policy engine must be deterministic for the same facts and policy version.
AI may propose explanations or classifications, but it must not silently
change a budget, skip a protected check, or invent a receipt.

## Product surfaces

The default surface should be a Work Item control view, not a dashboard.

It should answer:

- what is the current task;
- what is happening now;
- what has been spent;
- what is the next recommended action;
- what has been proven;
- what remains unproven;
- what decision is needed from the developer.

Deeper surfaces:

### Run Control

Live state, budget, iteration count, tool activity, loop warnings, pause/stop,
and current next action.

### Verification Advisor

Selected, deferred, and required checks with cost, evidence, and policy reason.

### Change Economics

Cost per accepted change, correction cost, retry cost, verification efficiency,
and model/worker fitness by project and task type.

### Decision Inspector

Full Decision Receipt and evidence chain for one recommendation.

### Project Learning

Reviewed project-specific rules such as “changes in this subsystem require the
contract suite” or “this model has a high correction rate for this task type.”

Generic Tokens, Audit, and Dashboard routes should not remain competing primary
destinations.

## Current feature disposition

### Keep and elevate

- Work Items and acceptance criteria;
- bounded context and context token estimates;
- worktrees and changesets;
- Runs and execution evidence;
- guarded checks and verification receipts;
- Engineering Intelligence event ledger;
- project knowledge with provenance;
- RepoPilot integration as a proof provider;
- local-first security and privacy boundaries.

### Merge into the control plane

- Models and Cost -> routing, preflight estimate, and budget policy;
- Tokens -> runtime evidence and Change Economics, not a separate destination;
- Audit -> Decision Inspector and evidence chain;
- Orchestrate -> runtime infrastructure behind Work and Run Control;
- Outcomes -> accepted-change and correction-cost analytics;
- Playbooks -> project policies and Work Item templates;
- memory -> scoped, reviewed Project Learning.

### Retire as primary product concepts

- generic Dashboard as a standalone home;
- parallel Git and orchestration navigation;
- provider-first navigation;
- opaque productivity scores;
- any completion state based only on an agent's prose;
- automatic “green” state when a check ran zero tests or against a stale tree.

### Build

1. Run Control with preflight budget and stop conditions;
2. normalized worker adapter contract;
3. Decision Receipt;
4. Adaptive Verification Advisor;
5. independent verification receipt validation;
6. failure grouping and compact agent feedback;
7. cost per accepted change and correction-cost views;
8. repo-specific policy learning with human review;
9. circuit breakers for loops, budgets, and repeated no-progress attempts.

## Phased product sequence

### Phase 0 — truthful evidence

- unify existing token, run, context, check, and changeset records;
- bind usage and checks to Work Item, execution, attempt, context, and tree;
- expose missing/unknown fields honestly;
- ensure verification receipts prove which command ran on which tree.

### Phase 1 — guided decisions

- preflight estimate;
- visible budget and stop policy;
- recommendation-first test advisor;
- Decision Receipt;
- cost per accepted change read model;
- live Run Control surface.

### Phase 2 — safe intervention

- loop and no-progress detection;
- pause/stop/report-only transitions;
- grouped test failures and bounded feedback;
- policy-controlled model rerouting;
- explicit verification debt and commit gate behavior.

### Phase 3 — project learning

- project-specific check selection history;
- model fitness by task type;
- context reuse and correction-cost recommendations;
- reviewed learning proposals, never silent policy mutation.

## Success criteria

The product direction is working when a user can:

1. see an estimated cost and stop policy before starting a run;
2. identify which worker, model, retry, or tool loop consumed the spend;
3. stop a runaway run without losing its partial evidence;
4. understand why a check was selected or deferred;
5. verify that every reported check ran against the intended tree;
6. see all unresolved verification debt before commit;
7. compare cost and correction effort for accepted changes, not only sessions;
8. use a cheaper model when project evidence says it is sufficient;
9. use RepoPilot evidence without RepoDesk duplicating its analysis;
10. operate locally without uploading source, prompts, or raw transcripts by
    default.

Initial product hypotheses to test with design partners:

- preflight estimates reduce cost anxiety and abandoned agent runs;
- loop controls prevent meaningful wasted spend without reducing accepted work;
- receipt-based verification is more trusted than agent summaries;
- cost per accepted change is more actionable than token totals;
- users prefer recommendations first and automation after the system proves its
  calibration on their repository.

## Non-goals

- building a replacement for Hermes, Codex, Claude Code, or Cursor;
- owning provider memory, skills, messaging, or cloud channels;
- replacing CI or project-specific test infrastructure;
- claiming mathematical confidence where only heuristics exist;
- optimizing for fewer tests at the expense of evidence;
- collecting raw prompts for surveillance or a global productivity ranking;
- expanding provider integrations before the evidence and policy loop is useful.

## Review questions before implementation

1. Is “Verification Governor / AI Engineering Control Plane” the right product
   category and language?
2. Should the first design-partner audience be solo power developers and small
   teams, rather than enterprise engineering management?
3. Should Phase 1 remain recommendation-first, with automation gated by
   explicit project policy?
4. Does `DecisionReceipt` express the product's unique object clearly enough,
   or should it be named `Verification Decision`, `Run Decision`, or another
   term?

Implementation must not begin until this document is reviewed and the answers
are resolved.
