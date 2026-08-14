# RepoDesk Next Development Plan

## Purpose

This file is the **execution sequence**, not a second product thesis.

Use these documents together:

- [`PRODUCT_CONVERGENCE_AUDIT_2026-08.md`](PRODUCT_CONVERGENCE_AUDIT_2026-08.md) — current-state product/feature/IA/design audit and current product decisions.
- [`REPODESK_2_PRODUCT.md`](REPODESK_2_PRODUCT.md) — durable product foundation and RepoDesk/SubRadar boundary.
- this file — what to implement next, in order, with acceptance contracts.

If an older roadmap conflicts with the current convergence audit, the convergence audit wins.

---

# Product contract

RepoDesk is a **local-first, agent-agnostic engineering control and evidence workspace for trustworthy software change**.

Canonical workflow:

```text
Scope -> Prepare -> Execute -> Review -> Verify -> Finish
```

Canonical trust graph:

```text
WorkItem
  -> Goal
  -> Scope
  -> ContextManifest
  -> RoutingDecision
  -> ExecutorRun
  -> ChangeSet
  -> ReviewDecision
  -> VerificationReceipt
  -> Commit
  -> Outcome
  -> Knowledge
```

Five owning product surfaces:

- Work
- Code
- Changes
- Runs
- Projects

Settings is a global utility surface, not a sixth engineering workflow.

The next phase is **convergence before expansion**.

---

# Current baseline

Already implemented and therefore not roadmap items by themselves:

- SQLite state/migrations and safe restore validation/rollback;
- hash-chained canonical engineering event journal;
- typed engineering event projection into SQLite;
- deterministic bounded memory retrieval and pinned-budget enforcement at execution guard;
- bounded context pipeline and context provenance;
- reviewed Engineering Knowledge lifecycle;
- deterministic routing with hard cost blockers;
- provider/model-aware cost accounting;
- local/cloud/coding-agent executor separation;
- isolated worktree execution for supported coding agents;
- changeset review and verification receipts;
- stable Code tab document identity;
- feature-local lazy error containment;
- hardened provider URLs/path security/error-body capture;
- bounded, canonical Git workspace capture (#172): streaming NUL status, typed changed files, bounded status/diff/process output;
- CI, Playwright, native Tauri E2E, cargo-deny, gitleaks, source-size ratchet, release SBOM and immutable action pins.

Improve these foundations where an invariant below requires it. Do not recreate them under new names.

---

# Cut A — Canonical evidence authority

## Priority

P0

## Problem

`crates/repodesk-core/src/audit.rs` still maintains a second live hash-chained JSONL trail while the canonical engineering evidence journal is SQLite.

A system that claims traceability cannot have two competing live histories.

## Work

- inventory every `audit.rs` writer/caller;
- map useful audit actions to canonical engineering event types/payloads;
- stop new writes to `~/.repodesk/audit/audit_trail.jsonl`;
- preserve JSONL only as explicit legacy migration/import or export;
- add migration/deduplication behavior for historical audit rows if required;
- expose technical audit/debug views as projections of canonical events;
- redirect/remove `AuditTab` after migration;
- ensure Work Item, run, ChangeSet, receipt, commit, outcome and knowledge IDs are queryable first-class metadata.

## Acceptance

- exactly one authoritative live engineering event writer exists;
- normal UI history/audit is rebuildable from canonical state;
- legacy JSONL can never silently become newer than SQLite;
- corrupt legacy input cannot mutate canonical state without explicit validation;
- chain/integrity tests cover migration and canonical reads.

---

# Cut B — Prepare context fail-closed

## Priority

P0

## Problem

`context.rs` still converts legacy Memory Brain retrieval errors to `None` with `.ok()` and can silently fall back to `memory.md`.

Execution guard semantics are stricter than Prepare semantics, so the UI can construct a context artifact that looks usable before a later blocker appears.

## Work

- remove `.ok()` retrieval swallowing;
- propagate structured retrieval failure;
- fail Context Manifest construction when required/pinned memory cannot fit;
- permit legacy `memory.md` only under an explicit migration condition such as “no structured records exist”, never after a retrieval failure;
- record legacy fallback identity/provenance in the Context Manifest;
- align Prepare readiness and execution guard predicates;
- keep final rendered context under the hard configured ceiling;
- preserve replayability with explicit time-dependent inputs.

## Acceptance

- Prepare cannot report context ready when execution would block on the same context invariant;
- every source and fallback is attributable;
- pinned constraints are either included or construction fails explicitly;
- same inputs plus explicit clock yield the same selection decision.

---

# Cut C — ChangeSet as the trust authority

## Priority

P0/P1

## Product rule

**Git is an implementation substrate; ChangeSet is the product abstraction.**

Changes becomes the sole mutable owner of review, verification and commit readiness.

## Work

### ChangeSet Passport

Record/render:

- ChangeSet ID;
- Work Item;
- baseline;
- producer run;
- attribution/isolation strength;
- files/hunks;
- scope/protected-path state;
- human review decision;
- verification receipts;
- acceptance coverage;
- commit identity;
- knowledge produced.

### Acceptance Evidence Matrix

Represent every required criterion explicitly:

```text
criterion -> evidence/receipt -> state -> stale reason
```

### Verification binding

Bind receipts to the exact relevant state:

- ChangeSet/tree/index/head fingerprint;
- check/tool identity and material version;
- run/workspace;
- bounded evidence artifact hash;
- start/end/result.

Changing relevant state must make the old receipt visibly stale.

### Safe Commit Manifest

Before commit, render a deterministic gate summary:

```text
Work Item
ChangeSet
scope/protected paths
review decision
required current receipts
unattributed changes
commit message
```

## Acceptance

From Changes alone the user can answer:

1. what changed;
2. who/what produced it;
3. whether attribution is exact or derived;
4. whether the change is in scope;
5. whether a human accepted it;
6. whether required verification is current for this exact state;
7. whether commit is allowed and why.

---

# Cut D — Information architecture convergence

## Priority

P1

## Goal

The normal lifecycle must require only:

- Work;
- Code;
- Changes;
- Runs;
- Projects.

## Retire / redistribute

### Git route

- redirect to Changes;
- move raw Git diagnostics into a technical inspector;
- keep Git backend primitives.

### Orchestrate route

Keep orchestration engine, remove orchestration as a competing product workflow:

- plan/routing/approval -> Work;
- executor availability -> Projects;
- worktree/run/recovery -> Runs;
- diff/proof/review/verify -> Changes;
- knowledge proposals -> Finish/Projects Knowledge.

### Runs

Keep execution evidence only:

- route/executor/context/worktree/log/cost/recovery;
- immutable links to ChangeSet/receipts.

Move mutable review/verification/commit controls to Changes.

### Models / Tokens / Models & Cost

- configured availability/policy/budgets -> Projects;
- actual per-run usage/cost -> Runs;
- aggregates/cost-to-accepted-change -> Projects evidence.

Remove the three routes when consumers migrate.

### Outcomes

- remove “brain/adaptive router” UI;
- project-level routing/executor evidence -> Projects;
- hard policy remains authoritative over learned evidence.

### System Registry

- machine-wide discovery/credentials -> Settings;
- project-enabled capabilities/integrations -> Projects;
- execution capability receipt -> Runs.

### Playbooks

Replace generic route shortcuts with **Work Templates** owned by Projects.

A Work Template may define:

- goal skeleton;
- default scope rules;
- acceptance criteria;
- required checks;
- execution policy;
- capability preference;
- knowledge tags;
- finish checklist.

### Dashboard

Delete.

### Debug / Audit

Developer-mode/technical detail only; not normal product navigation.

### Knowledge

Keep the capability. Contextualize it:

- selected knowledge -> Work Prepare;
- evidence-backed proposal -> Finish;
- source-specific context -> Code;
- lifecycle CRUD -> Projects.

## Navigation cleanup

RepoDesk currently has overlapping navigation in:

- activity rail;
- WorkspaceSidebar Related links;
- WorkSurface Related links.

After migration:

- activity rail owns destination navigation;
- context sidebar owns current-surface structure/status;
- inspector owns deep evidence/detail;
- command palette owns shortcuts/deep links.

## Acceptance

- no normal workflow requires a hidden/legacy route;
- every important concept has exactly one owning surface;
- legacy route IDs redirect safely until removal;
- no second review/verification authority remains.

---

# Cut E — Projects as durable engineering configuration

## Priority

P1

## Problem

Projects is currently mostly a registry, while Settings owns project connection, project type/language, project imports, provider choices and some project memory/guideline concerns.

## Work

Move project-scoped concerns into Projects:

- repository path/type/language;
- checks;
- context rules;
- budgets and execution policy;
- enabled executor capabilities;
- project commands;
- Engineering Knowledge lifecycle;
- Work Templates;
- project imports/integrations;
- routing/executor evidence aggregates.

Keep Settings global:

- credentials/keychain;
- machine-wide provider endpoints/discovery;
- IDE/application preferences;
- updates;
- privacy/telemetry;
- developer mode.

## Acceptance

Settings contains no Work Item/project engineering policy and Projects can completely explain how a repository is configured for trustworthy change.

---

# Cut F — Design-system convergence

## Priority

P1

## Problem

The UI still contains multiple visual generations and local dialects:

- route-specific panel structures;
- `work-focus-polish.css` / `work-visual-language.css` style generations;
- shared `routing-feature.css` used by unrelated product surfaces;
- nested subnav patterns created by routes that should not exist;
- inline layout styles;
- raw status hex maps;
- versioned class names such as `work-workbench-v3` and `knowledge-workspace-v2`.

## Target layers

```text
foundation/tokens
  -> primitives
  -> shell/workbench
  -> domain components
  -> small feature-local exceptions
```

## Semantic primitives

Standardize:

- `StatusBadge`;
- `EvidenceState`;
- `PanelHeader`;
- `EmptyState`;
- `LoadingState`;
- `ErrorState`;
- `InspectorSection`;
- `ActionBar`;
- `Metric` only when the number changes a decision.

## Ratchet

After the first migration establish non-regression checks for:

- raw hex in TSX outside approved tokens;
- new inline layout styles;
- new `*-vN` visual classes;
- new route-wide polish files;
- feature CSS byte growth above baselines.

Do not fail on all historical debt immediately. Freeze growth first, then lower baselines.

## Acceptance

- five primary routes share one semantic status/evidence language;
- no new versioned visual generation is introduced;
- retired route CSS can be deleted without restyling unrelated screens;
- loading/error/empty/action patterns are not rebuilt ad hoc per feature;
- visual regression coverage exists for Work, Code, Changes, Runs and Projects.

---

# Cut G — Execution attribution and isolation truth

## Priority

P1

## Problem

Pre/post Git state is useful but does not prove exact producer attribution when the initial workspace is already dirty.

## Attribution states

Expose a typed evidence strength, for example:

- `exact_isolated`;
- `exact_clean_workspace`;
- `derived_pre_post`;
- `unattributed`.

Never collapse these into a Boolean claim.

## Work

- use clean isolated worktrees for supported write-capable executors;
- record baseline tree/worktree identity;
- derive exact produced ChangeSet;
- preserve weaker attribution states for unsupported/manual paths;
- surface attribution strength in Changes;
- couple verification/commit policy to attribution when project policy requires it.

## Acceptance

RepoDesk never claims an exact producer without evidence sufficient to prove it.

---

# Cut H — Editor/session durability

## Priority

P1

Stable Code tab identity is implemented. The remaining durability issue is shutdown coordination.

## Work

- replace direct native tray Quit -> `app.exit(0)` behavior with a frontend/native shutdown handshake;
- flush dirty drafts before exit;
- ensure the mechanism works even if Code is not the currently mounted route;
- include a bounded fallback so broken frontend state cannot make the application impossible to exit;
- add regression tests for edit -> immediate tray Quit.

## Acceptance

A dirty edit survives every supported normal quit/close path and duplicate display paths never share editor identity.

---

# Engineering Intelligence after convergence

Do not build another analytics dashboard.

Keep independent evidence-backed dimensions:

- context compactness;
- scope adherence;
- verification success;
- retry/correction cost;
- accepted-change ratio;
- knowledge reuse;
- executor fan-out;
- redundant execution;
- time to verified change;
- cost to verified accepted change;
- Structural Complexity Risk.

Build intelligence that changes a future decision:

- Context Manifest delta between attempts;
- correction-loop map;
- repeated rediscovery / knowledge debt;
- unnecessary verification rerun detection;
- pre/post change-risk delta;
- trust comparison between two attempts/executors.

Do not create a single “AI efficiency score” until its semantics are defensible.

---

# Differentiating features after P0/P1 convergence

## ChangeSet Passport

One compact object linking intent, producer, diff, review, receipts, commit and learned knowledge.

## Why inspector

Explain:

- why a context source was selected/excluded;
- why an executor was eligible/selected;
- why a file was writable;
- why verification is stale;
- why commit is blocked.

## Verification replay

Replay a receipt against the current exact ChangeSet and explain whether result/state/environment changed.

## Scope drift map

Distinguish explicitly allowed, dependency-related, protected, attributed and unattributed changed files.

## Review handoff packet

Summarize intent, scope, risky hunks, findings, evidence gaps and current receipts for human review.

## Outcome-to-knowledge closure

At Finish create only evidence-backed knowledge candidates tied to the exact Work Item/receipt/commit that motivated them.

---

# Release readiness

Product convergence does not replace release hardening.

## P0 decisions

- choose LICENSE/business model deliberately;
- configure updater signing and validate a real signed updater canary;
- configure macOS signing/notarization;
- enable GitHub private vulnerability reporting.

## P1

- Windows signing;
- branch protection/required checks;
- legal/privacy review for cloud integrations;
- explicit telemetry stance.

## Already in place / maintain

- immutable GitHub Action pins where practical;
- release SPDX SBOM;
- cargo-deny;
- gitleaks;
- CI/native E2E/release verification.

---

# Product validation

Initial ICP:

- senior/staff developers;
- small engineering teams already using multiple coding agents/models;
- teams experiencing scope drift, opaque context, unsafe/unreviewable changes, duplicated work, review fatigue and project-knowledge loss.

Product promise:

> RepoDesk makes a software change bounded, attributable, reviewable, verifiable and recoverable regardless of which executor produced it.

Target demo:

```text
Connect repo
  -> Create Work Item
  -> define scope/acceptance
  -> inspect Context Manifest
  -> select/justify executor
  -> execute
  -> review exact ChangeSet
  -> satisfy Evidence Matrix
  -> Safe Commit Manifest
  -> commit
  -> capture reviewed knowledge
```

Before another broad feature wave:

- recruit 10–15 design partners;
- measure which guardrails/evidence are inspected, overridden or ignored;
- test willingness to pay for trustworthy-change governance rather than token resale;
- build a clear “Why RepoDesk if I already use Cursor/Codex/etc.?” comparison around evidence/control.

---

# Do not do next

Avoid:

- another top-level AI chat;
- another dashboard;
- another model/provider destination;
- learned routing that can bypass hard constraints;
- unrestricted autonomous commit/push/merge;
- unbounded repository/context/process output;
- another state/event store;
- another visual polish generation;
- a broad VS Code replacement;
- fake formal Big-O claims;
- broad feature expansion before Cuts A–G establish one coherent trustworthy-change path.

---

# Definition of the next phase complete

The convergence phase is complete when:

1. SQLite is the only live canonical engineering evidence ledger.
2. Prepare and Execute enforce the same fail-closed context rules.
3. ChangeSet is the sole mutable review/verification/commit authority.
4. receipt staleness is bound to exact relevant state.
5. attribution strength is explicit and truthful.
6. the normal lifecycle requires only Work, Code, Changes, Runs and Projects.
7. Settings contains global configuration rather than project workflow policy.
8. Engineering Knowledge is reviewed, attributable and contextual.
9. design/status/evidence primitives have one semantic vocabulary and no new versioned style layers.
10. the 90-second demo is one coherent trustworthy software-change story.

Only then should the next broad differentiating feature wave begin.
