# RepoDesk Product Convergence Audit — August 2026

## Status

Current-state audit after the Git workspace convergence work merged in #172.

This document is intentionally opinionated. Its purpose is to reduce product entropy, remove duplicate concepts, and make the next implementation sequence easier to reason about.

The audit is grounded in the current repository, especially:

- `apps/desktop/src/app/tabs.tsx`
- `apps/desktop/src/app/WorkspaceSidebar.tsx`
- `apps/desktop/src/features/work/*`
- `apps/desktop/src/features/changes/*`
- `apps/desktop/src/features/git/*`
- `apps/desktop/src/features/history/*`
- `apps/desktop/src/features/orchestrate/*`
- `apps/desktop/src/features/knowledge/*`
- `apps/desktop/src/features/models-cost/*`
- `apps/desktop/src/features/models/*`
- `apps/desktop/src/features/tokens/*`
- `apps/desktop/src/features/outcomes/*`
- `apps/desktop/src/features/audit/*`
- `apps/desktop/src/features/playbooks/*`
- `apps/desktop/src/features/system/*`
- `apps/desktop/src/features/settings/*`
- `crates/repodesk-core/src/context.rs`
- `crates/repodesk-core/src/audit.rs`
- `crates/repodesk-core/src/persistence/event_journal.rs`
- `crates/repodesk-core/src/git_workspace/*`
- `docs/REPODESK_2_PRODUCT.md`
- `docs/NEXT_DEVELOPMENT_PLAN.md`

---

# 1. Executive verdict

RepoDesk has crossed an important line: the backend contains enough real engineering primitives that the product no longer needs to present itself as an AI controller, provider cockpit, or collection of agent utilities.

The strongest product is now much narrower and more defensible:

> **RepoDesk is a local-first engineering workspace that makes software changes bounded, attributable, reviewable, verifiable, recoverable, and reusable — regardless of whether the work was performed by a human, a coding agent, a model, a script, or CI.**

The repository already contains many of the hard primitives required for this thesis:

- Work Items and an explicit phase model;
- typed scope/acceptance contracts;
- bounded context construction;
- reviewed project engineering knowledge;
- deterministic routing and hard policy/cost blockers;
- multiple executor types;
- isolated worktree execution for write-capable coding agents;
- Git state, typed changesets, diff review and commit gates;
- verification receipts;
- canonical hash-chained SQLite engineering events;
- model-aware usage and cost;
- guarded execution, credentials, backup/restore and release gates;
- an IDE-like desktop shell with Code, terminal, problems and repository navigation.

The largest remaining problem is therefore **not missing features**. It is **conceptual duplication**.

RepoDesk currently exposes several historical generations of the product at the same time:

1. the original AI/runtime cockpit;
2. an orchestration console;
3. an IDE/workbench;
4. a Git dashboard;
5. an audit dashboard;
6. the newer trustworthy-change workflow.

The result is that the backend is becoming more coherent while the UI still asks users to learn parallel nouns and parallel paths to the same evidence.

The next product phase should be a **convergence phase, not a feature-expansion phase**.

---

# 2. North Star

## 2.1 The product object is the change, not the agent

The central graph is:

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

The user should be able to move forward and backward through this graph without changing mental models.

The primary workflow remains:

```text
Scope -> Prepare -> Execute -> Review -> Verify -> Finish
```

The strongest future form is:

```text
Intent
  -> explicit acceptance contract
  -> bounded context manifest
  -> policy/routing decision
  -> isolated executor workspace
  -> typed ChangeSet
  -> review findings/decision
  -> verification receipts bound to exact state
  -> safe commit
  -> outcome
  -> reviewed durable project knowledge
```

## 2.2 Five questions define whether a feature belongs

Every product surface must help answer at least one of these:

1. **What are we trying to change?**
2. **What context and policy constrain the change?**
3. **What exactly changed, and which execution caused it?**
4. **What evidence says the change meets acceptance and is safe?**
5. **What gets committed and learned?**

If a feature answers none of these, it should be developer tooling, settings, or removed.

If two surfaces answer the same question, one must own the concept and the other must link to it by identity rather than render another mutable projection.

## 2.3 Product laws

The following should become explicit design laws:

- **Agents are executor metadata, not navigation objects.**
- **Activity is not success. Verified evidence is success.**
- **Git is an implementation substrate; ChangeSet is the product abstraction.**
- **Logs are observation; receipts are evidence.**
- **One concept has one owning surface. Other surfaces link to it by stable ID.**
- **No “brain” mysticism. Routing is a policy/evidence decision with inspectable reasons.**
- **No fake confidence. Heuristic intelligence must expose confidence and missing evidence.**
- **Anything repository-controlled is bounded: context, status, diffs, process output, logs, evidence and memory.**
- **A dashboard is useful only if it changes the next engineering decision.**
- **The next safe action matters more than another metric.**

---

# 3. Current information architecture: too many products under five icons

`apps/desktop/src/app/tabs.tsx` correctly defines five primary destinations:

- Work
- Code
- Changes
- Runs
- Projects

But it also retains twelve contextual/hidden destinations:

- Knowledge
- Git
- Orchestrate
- Playbooks
- Models & Cost
- Settings
- System Registry
- Dashboard
- Debug
- Models
- Tokens
- Outcomes
- Audit

This means the rail looks focused while the underlying product taxonomy remains broad.

The important distinction is not “primary vs hidden”. The important distinction is **whether the destination represents a unique user job**.

Several do not.

## Recommended ownership model

### Work owns

- goal;
- Work Item identity;
- scope;
- protected paths;
- acceptance criteria;
- context readiness;
- execution policy preview;
- route/executor decision preview;
- approvals required before execution;
- current workflow phase;
- current blocker;
- next safe action.

### Code owns

- repository browsing;
- manual reading/editing;
- symbols/search;
- diagnostics tied to source;
- related tests/rules/knowledge for a selected code location;
- terminal/problems when performing manual engineering work.

### Changes owns

- exact ChangeSet;
- changed files;
- Git delta as implementation evidence;
- attribution/provenance;
- scope violations;
- review findings;
- human review decision;
- verification status;
- acceptance evidence matrix;
- staging/commit readiness;
- commit action.

### Runs owns

- executor invocation;
- provider/model/agent identity;
- route reason;
- exact input context reference;
- stdout/stderr/log summaries;
- token/cost/latency;
- isolation/worktree identity;
- recovery state;
- execution failures;
- immutable links to produced ChangeSets and receipts.

Runs should observe review/verification evidence, not own mutable review/verification decisions.

### Projects owns

- project registry;
- repository configuration;
- project rules and budgets;
- executor/provider availability for the project;
- project engineering knowledge;
- reusable Work templates/playbooks;
- project-specific commands/checks;
- routing/executor evidence aggregates;
- integrations relevant to engineering work.

### Settings owns only application-global settings

- appearance/IDE preferences;
- global credential storage;
- global provider installation/discovery where truly machine-wide;
- update/privacy/telemetry preferences;
- developer mode.

Project configuration should not be hidden inside global Settings.

---

# 4. Feature disposition matrix

## 4.1 Work — KEEP, simplify and strengthen

**Decision: KEEP as the product spine.**

`WorkSurface` is directionally correct, but it currently contains several historical layers:

- an internal command rail;
- phase progress;
- Work Intelligence summary;
- Contract/Context/Intelligence inspector navigation;
- a second “Related” navigation list to Code/Changes/Runs/Knowledge;
- an “Advanced orchestration” escape hatch;
- the main `WorkTab` workflow.

### Keep

- Work Item identity;
- phase state;
- contract inspector;
- context inspector;
- explicit blocker/next-action state.

### Change

- Replace “AI Usage Intelligence” with **Decision Evidence** or phase-specific evidence. Context compactness, executor fan-out and token efficiency are useful only when they explain a decision or a failure.
- Remove the Work-local “Related” mini-navigation. The application rail and contextual inspector already provide navigation.
- Remove “Advanced orchestration” as a separate route. Execution strategy belongs in the Execute phase.
- Remove versioned naming such as `work-workbench-v3` once the style migration begins.
- Stop carrying legacy “AI packet” vocabulary when the object is a Context Manifest.

### Add

A single compact **Change Readiness** header on Work:

```text
Scope        ready / blocked
Context      ready / blocked
Policy       ready / blocked
Executor     selected / approval needed
Changes      none / produced
Review       pending / accepted / rejected
Verification pending / passed / failed / stale
Commit       blocked / ready / committed
```

Only the current and blocking stages need visual prominence.

---

## 4.2 Code — KEEP, resist becoming a generic IDE clone

**Decision: KEEP.**

The editor is valuable because RepoDesk needs a place for manual intervention and evidence inspection without forcing the developer into a different application.

### Keep

- repository tree;
- stable document tabs;
- editor;
- search/symbol navigation;
- terminal/problems;
- code-level findings.

### Avoid

- competing with VS Code extension breadth;
- adding generic chat as the center of Code;
- duplicating ChangeSet review inside the editor;
- showing project-wide dashboards in the Code surface.

### Add later

- selected-symbol evidence inspector;
- related Work Item(s);
- related accepted project knowledge;
- related verification failures;
- exact changed-line provenance (“changed by run X for work item Y”);
- LSP/HIR-backed structural intelligence.

---

## 4.3 Changes — KEEP and make it the sole review/verification authority

**Decision: KEEP and expand ownership.**

Current `ChangesTab` already combines the strongest useful parts of the old Git and review experiences:

- branch/dirty state;
- staged/unstaged/untracked grouping;
- diff/file preview;
- findings;
- governance;
- commit gate;
- scope/protected/unattributed state;
- verification/evidence.

This is the correct product abstraction.

### Changes should become the answer to

> “What will enter the repository, who caused it, and what proves it is acceptable?”

### Add

#### Evidence Matrix

Acceptance criteria should be rows, not prose hidden in Work:

```text
Criterion                 Evidence                     State
API remains compatible    cargo test + ABI check       PASS
No protected files        ChangeSet scope receipt      PASS
No new secrets            gitleaks receipt             PASS
Latency <= target          benchmark receipt            UNKNOWN
```

A Work Item cannot Finish if required rows are unresolved.

#### ChangeSet provenance graph

For a selected file/hunk:

```text
Work Item -> Run -> Context Manifest -> ChangeSet -> Review -> Verification
```

#### Receipt staleness

Verification must show whether it applies to the **current exact state**. If the index/tree/ChangeSet changes, old receipts become visibly stale rather than remaining green.

---

## 4.4 Git — RETIRE AS A ROUTE

**Decision: REMOVE product route.**

Current `GitTab` duplicates the core of Changes:

- branch;
- dirty count;
- file groups;
- diff;
- diff stat;
- raw snapshot.

This is not a second user job.

### Migration

- keep Git capture/backend APIs;
- keep branch/worktree implementation details;
- move raw diagnostic snapshot behind a developer/diagnostic disclosure in Changes;
- deep links to `git` should redirect to `changes` with an optional inspector/detail selector;
- delete the separate route after migration coverage exists.

**Product rule:** Git state supports ChangeSet reasoning; it does not compete with ChangeSet as a navigation concept.

---

## 4.5 Runs — KEEP, narrow authority

**Decision: KEEP, but make it execution evidence only.**

Current Runs/History includes run evidence plus Provider Outcomes and Raw Audit, and detailed views can edit review/verification/acceptance state.

That makes Runs a second workflow owner.

### Keep

- executor identity;
- routing decision;
- context fingerprint/reference;
- worktree/isolation;
- process result;
- logs;
- cost/tokens/latency;
- recovery operations;
- links to ChangeSet and verification receipts.

### Move out

- mutable review decision -> Changes;
- mutable acceptance evidence -> Changes/Work;
- commit state authority -> Changes;
- Provider Outcomes -> Projects analytics/routing evidence;
- Raw Audit -> developer evidence inspector.

Runs should be an immutable-ish forensic view of **what happened**, not another place to decide **whether the change is acceptable**.

---

## 4.6 Orchestrate — RETIRE AS A ROUTE, KEEP THE ENGINE

**Decision: DELETE the product destination; preserve backend orchestration.**

`OrchestrateTab` is a parallel super-workflow containing plan, approvals, execution, isolated worktrees, review, workers, recovery, cost, proof, memory proposals and runs.

That duplicates the canonical lifecycle almost one-for-one.

### Redistribute

- plan/routing preview -> Work / Prepare + Execute;
- execution approval -> Work / Execute;
- executor availability -> Projects;
- isolated worktree status -> Runs details;
- worktree recovery -> Runs recovery action;
- diff/proof -> Changes;
- verification -> Changes;
- durable knowledge proposal -> Finish/Knowledge;
- cost -> run evidence + Projects aggregate.

The orchestration engine is valuable. The orchestration **page** is historical architecture leaking into product navigation.

---

## 4.7 Knowledge — KEEP, but contextualize

**Decision: KEEP the capability; reduce route prominence over time.**

The newer Engineering Knowledge implementation is one of RepoDesk's differentiators because it has:

- explicit provenance;
- candidate vs accepted state;
- human review;
- lifecycle/reconfirmation;
- evidence;
- fail-closed exclusion when review is required.

That is much better than generic “memory”.

### Product placement

- Work / Prepare: show only knowledge selected for this Work Item and why;
- Work / Finish: propose knowledge from verified outcomes;
- Code: show knowledge relevant to selected subsystem/symbol;
- Projects: full knowledge lifecycle management.

A standalone Knowledge deep link can remain for advanced CRUD, but it should conceptually belong to Projects rather than be another equal product mode.

### Remove vocabulary debt

- `memory` route ID may stay temporarily for compatibility;
- visible UI should say Engineering Knowledge, not Memory Brain;
- legacy `memory.md` should be migration input, not a silent fallback truth source.

---

## 4.8 Playbooks — MERGE INTO WORK TEMPLATES / PROJECTS

**Decision: RETIRE as a generic navigation-shortcut product.**

Current playbooks are primarily editable shortcuts that open other routes. Their target list even includes destinations that should themselves disappear (`models-cost`, `orchestrate`).

A shortcut catalog is not a strong product primitive.

### Better abstraction: Work Templates

A template should encode useful engineering intent:

```text
Template
- title
- goal skeleton
- default scope rules
- acceptance criteria
- required checks
- execution policy
- preferred capability class
- knowledge tags
- optional finish checklist
```

Examples:

- Dependency upgrade
- Security fix
- Add API endpoint
- Database migration
- Performance investigation
- Refactor without behavior change

Templates live under Projects and instantiate a Work Item.

A command-palette shortcut can still open any route without requiring a persisted “Playbook” domain object for navigation.

---

## 4.9 Models + Tokens + Models & Cost — MERGE BY OWNERSHIP

**Decision: REMOVE all three as product destinations.**

`ModelsCostTab` currently nests `ModelsTab` and `TokensTab`, while both legacy routes still exist independently. This is explicit duplication.

The underlying data is useful; the surfaces are not.

### New ownership

**Projects / Execution policy**

- available executors/providers;
- enabled models;
- capability classes;
- configured budgets/rate cards;
- default policy.

**Runs**

- actual executor/model used;
- actual/estimated tokens and cost;
- latency;
- fallback/reroute evidence.

**Projects / Evidence aggregates**

- cost by Work Item / executor / model;
- success/retry rate;
- cost of accepted change;
- correction/rework cost.

The product should care about **cost of a trustworthy accepted change**, not just tokens consumed.

---

## 4.10 Outcomes — MERGE INTO PROJECT EXECUTION EVIDENCE

**Decision: REMOVE route and “brain” language.**

Current Outcomes copy describes “what the brain learned” and an “adaptive router”. This conflicts with the inspectable deterministic policy model.

### Replace with Routing Evidence

A project-level view may show:

- executor capability match rate;
- successful verified changes by executor/model;
- retry/rejection rate;
- median correction cost;
- scope violation rate;
- user overrides;
- cost per accepted ChangeSet;
- confidence/sample size.

Any learned residual must be bounded and subordinate to hard policy.

No mystical adaptation. Show the evidence and the exact effect it can have.

---

## 4.11 Audit — RETIRE LEGACY WRITER AND ROUTE

**Decision: P0 correctness issue.**

RepoDesk currently has two audit/evidence systems:

1. canonical SQLite engineering event journal;
2. `crates/repodesk-core/src/audit.rs`, which separately writes `~/.repodesk/audit/audit_trail.jsonl` with its own SHA-256 hash chain.

`AuditTab` reads the second one through Tauri audit commands.

This violates the one-canonical-ledger principle and makes the product incapable of answering which history is authoritative.

### Required migration

- stop all new JSONL audit writes;
- map any still-useful audit actions to canonical typed/event-journal events;
- add one-time/import-on-read legacy migration if historical users need the old trail;
- verify/deduplicate imported evidence;
- make SQLite the only ongoing authoritative writer;
- expose audit/debug projection from canonical events;
- remove `AuditTab` as a product destination.

JSONL may remain as export or explicit migration input, never as a second live ledger.

---

## 4.12 System Registry — MERGE INTO PROJECTS / SETTINGS

**Decision: REMOVE route.**

Current System Registry mixes:

- agents;
- capabilities;
- peripherals/modules;
- installed AI tools;
- endpoint discovery;
- MCP-like tooling;
- recommendations.

These are infrastructure/configuration concerns, not a standalone engineering workflow.

### Ownership

- machine-wide installed tools / credentials -> Settings;
- project-enabled executors/capabilities -> Projects;
- MCP/integrations -> Projects when scoped to engineering work, Settings when global;
- runtime capability receipt for a particular execution -> Runs.

Also remove visible “brain modules” language.

---

## 4.13 Settings — SPLIT GLOBAL VS PROJECT CONFIG

**Decision: KEEP route but substantially shrink it.**

Current Settings is overloaded with:

- API keys;
- secure keychain;
- project connection/setup;
- project type/language;
- IDE preferences;
- provider toggles;
- model/runtime URLs;
- preferred providers for patch/compression/review;
- custom providers;
- project AI import;
- project memory/guidelines.

This makes Projects mostly a registry while Settings owns actual project configuration.

That is backwards.

### Target

**Projects / Configure Project**

- repo path/type/language;
- checks;
- context rules;
- budgets/policy;
- enabled executor capabilities;
- project knowledge;
- Work templates;
- project imports/integrations.

**Settings**

- credentials;
- globally installed provider endpoints;
- app/IDE preferences;
- updates/privacy/telemetry;
- developer mode.

---

## 4.14 Dashboard — DELETE

**Decision: DELETE.**

A legacy at-a-glance page reintroduces cockpit thinking.

The Work surface should show the current engineering decision; Projects can show aggregate project health when needed.

Do not keep a dashboard merely because data exists.

---

## 4.15 Debug — DEVELOPER MODE ONLY

**Decision: KEEP capability, remove from normal product IA.**

Debug traces are valuable for RepoDesk development and support.

They should be reachable through:

- developer mode;
- diagnostics bundle;
- contextual “show technical details”.

They should not be part of normal engineering navigation.

---

# 5. Navigation audit

## Problem: RepoDesk currently has three navigation systems

1. global activity rail;
2. `WorkspaceSidebar` Related links;
3. WorkSurface's own Related links.

This creates repeated affordances for Code/Changes/Runs/Knowledge and forces the developer to decide which navigation layer is meaningful.

## Target shell

```text
┌────────────────────────────────────────────────────────────────────┐
│ project | work item | branch/change state | command palette        │
├─────┬──────────────────┬──────────────────────┬────────────────────┤
│rail │ context sidebar  │ owning work surface  │ evidence inspector │
│     │                  │                      │                    │
├─────┴──────────────────┴──────────────────────┴────────────────────┤
│ terminal | problems | output (primarily Code/manual work)          │
└────────────────────────────────────────────────────────────────────┘
```

### Activity rail

Only:

- Work
- Code
- Changes
- Runs
- Projects

Plus Settings as a utility action, not a sixth engineering surface.

### Context sidebar

Do not list another generic “Related” taxonomy.

It should answer context-specific questions:

**Work**
- current Work Item;
- phase;
- blockers;
- scoped files;
- selected executor.

**Code**
- project tree/search;
- current file/symbol context.

**Changes**
- ChangeSets / file groups;
- scope state;
- verification state.

**Runs**
- runs for active Work Item;
- filters/status.

**Projects**
- project sections/configuration.

### Inspector

Inspector owns details that are useful but should not become routes:

- Context Manifest;
- routing explanation;
- raw Git diagnostics;
- event provenance;
- receipt metadata;
- selected finding evidence;
- selected run details;
- selected knowledge provenance.

---

# 6. Design audit

## 6.1 Current design debt is primarily ownership debt, not color debt

The repository has shared components, but feature styles still accumulate local visual dialects and historical layers.

Examples observed in the current tree:

- `work-route.css`
- `work-focus-polish.css`
- `work-visual-language.css`
- `routing-feature.css` imported across conceptually unrelated product surfaces;
- feature-specific route CSS for many legacy pages;
- `secondary-subnav.css` created to support nested route-within-route patterns;
- `manual-import.css` reused by unrelated import/form surfaces;
- many inline `style={{ ... }}` values;
- direct hex status colors in orchestration code;
- versioned class names such as `work-workbench-v3` and `knowledge-workspace-v2`.

Versioned CSS/class names are a strong signal that old visual ownership was not removed when new visual ownership was introduced.

## 6.2 Remove versioned visual generations

Do not create:

- `v4` workbench;
- `new-new` panels;
- `polish-v3`;
- route-wide overrides that patch previous route-wide overrides.

Migration target:

```text
1. foundation/tokens
2. primitives
3. shell/workbench
4. domain components
5. small feature-local exceptions
```

When a consumer migrates, delete the old selector/layer.

## 6.3 Define semantic primitives

RepoDesk repeatedly hand-builds the same concepts.

Create/standardize primitives for:

### `StatusBadge`

Inputs:

```text
state: success | warning | danger | neutral | unknown | blocked | running
label
optional detail
```

No feature-local hex maps.

### `EvidenceState`

For:

- verified;
- stale;
- missing;
- failed;
- unknown;
- blocked.

This is more meaningful than generic green/yellow/red status.

### `PanelHeader`

Own:

- eyebrow;
- title;
- description;
- right-side metadata/action.

Current `panel-title-row` structures are repeated manually.

### `EmptyState`

One visual grammar for:

- no project;
- no Work Item;
- no runs;
- no changes;
- no knowledge;
- no evidence.

Copy changes, structure does not.

### `LoadingState` / `ErrorState`

The repository already has shared error components; migrate feature-local one-off paragraphs/notices into the shared pattern.

### `InspectorSection`

For provenance/evidence metadata instead of every feature inventing a two-column grid.

### `ActionBar`

For primary/secondary/destructive actions and disabled reason text.

### `Metric`

Use only when a number can inform a decision. Avoid decorative metric cards.

## 6.4 Stop nested dashboard cards

Many legacy surfaces use:

```text
hero panel
  -> panel
     -> summary grid
        -> route list
           -> pill
```

This is visually expensive and makes all information look equally important.

Trustworthy-change UI should have stronger hierarchy:

1. current decision/blocker;
2. evidence needed for that decision;
3. detail on demand.

## 6.5 Inline styles are a design-system leak

Inline styles in System/Orchestrate/Work and similar components should be migrated when touched.

Do not perform a blind repository-wide style rewrite first. Use a ratchet:

- no new raw hex in TSX;
- no new `style={{ margin... }}` for ordinary layout;
- no new versioned class suffixes;
- no new route-wide polish CSS;
- reduce counts as each owning surface is migrated.

## 6.6 CSS bundle size is a symptom

Recent desktop build output showed disproportionately large CSS chunks for the key workbench surfaces, especially Work and Code, plus a large eager CSS base.

The goal is not a cosmetic byte-budget competition. The useful interpretation is:

> the same product concepts are likely being styled by too many overlapping layers.

Add a CSS/source architecture ratchet after the first consolidation PR:

- report feature CSS byte size;
- report selectors containing version suffixes;
- report raw hex in TSX/CSS outside tokens;
- report inline style count;
- fail only on new regressions at first;
- lower baselines incrementally.

---

# 7. Backend correctness audit

## 7.1 Git workspace — substantially converged

The #172 slice establishes the correct direction:

- one NUL-delimited streaming status reader;
- typed changed files as canonical state;
- bounded branch/commit/status/diff output;
- bounded compatibility helpers;
- bounded diagnostic `raw_status` projection;
- separated `status`, `snapshot`, `diff`, and `process` responsibilities.

Next Git work should be product-level provenance, not another status parser.

## 7.2 Exact executor attribution still needs the strongest invariant

Pre/post status comparison is useful but insufficient when a workspace is already dirty before execution.

The strongest execution contract is:

```text
clean isolated worktree
  -> known baseline tree
  -> one execution
  -> exact produced tree/patch
  -> typed ChangeSet identity
```

Until every write-capable path can provide this, RepoDesk must state attribution confidence honestly.

Possible states:

- `exact_isolated`
- `exact_clean_workspace`
- `derived_pre_post`
- `unattributed`

Never collapse these into a Boolean “agent changed this”.

## 7.3 Prepare still has a fail-open legacy memory path

`context.rs` currently converts Memory Brain retrieval failure to `None` with `.ok()` and then falls back to `memory.md`.

This means an execution guard can fail closed later while Prepare still constructs a context artifact that looks valid.

Required:

- propagate retrieval errors;
- treat pinned overflow as a Context Manifest construction failure;
- fall back to legacy `memory.md` only under an explicit migration rule such as “no structured memory records exist”, never after retrieval failure;
- attach provenance to every fallback source;
- ideally remove the fallback after migration.

## 7.4 Canonical event ledger is not yet canonical everywhere

Typed engineering events have been moved toward SQLite, but `audit.rs` remains a live second writer.

This is a P0 data-model issue because “audit”, “engineering event”, “run evidence”, and “history” can diverge.

Required invariant:

> A user-facing engineering fact is either canonical state or a projection from canonical state. It is not independently rewritten into another authoritative log.

## 7.5 Verification receipts need exact-state semantics everywhere

A receipt should identify at least:

- Work Item;
- ChangeSet;
- run/workspace;
- command/check identity;
- exact tree/index/head fingerprint relevant to the check;
- tool/version when relevant;
- start/end/time;
- result;
- bounded evidence artifact hash;
- environment/capability notes when material.

If the exact relevant state changes, the receipt becomes stale.

## 7.6 `command_sandbox` language must match enforcement

Where RepoDesk classifies commands and paths but does not create actual OS-level isolation, UI/docs should call it:

- command policy;
- execution policy;
- capability boundary;
- guarded command execution.

Reserve “sandbox” for an executor/OS mechanism that actually enforces an isolation boundary.

## 7.7 Structural complexity must not pretend to be a prover

Reframe heuristic algorithmic output as **Structural Complexity Risk**.

Recommended result shape:

```text
Structure
- nested loops / traversals / recursion / sorting / allocations

Likely behavior
- likely O(...)
- plausible alternatives

Confidence
- high / medium / low

Missing evidence
- collection semantics
- call target complexity
- type information
- input bounds
```

Then progressively improve semantics using compiler/LSP adapters.

## 7.8 Core module boundaries should follow product ownership

Do not split crates just to reduce file size.

Enforce internal dependency direction first:

```text
domain/contracts
        ↓
ledger/evidence
        ↓
workspace/changes
        ↓
execution/policy
        ↓
intelligence/context/knowledge/routing
```

A feature should not import a historical subsystem merely because that subsystem once owned the UI.

---

# 8. Engineering Intelligence: what is worth building

The product should not become a telemetry dashboard.

Useful intelligence changes a future engineering decision.

## Keep as independent dimensions

- context compactness;
- scope adherence;
- verification success rate;
- retry/correction cost;
- accepted-change ratio;
- knowledge reuse;
- executor fan-out;
- redundant execution;
- time to verified change;
- cost to verified accepted change;
- structural complexity risk.

## Do not create one “AI efficiency score” yet

A composite number hides trade-offs and becomes impossible to trust.

Better:

> “This Work Item used 3 executors because the first two failed the same verification check; 48% of context was repeated; the final accepted change cost $1.42 and 17 minutes.”

That is actionable.

## New differentiated intelligence ideas

### Context delta

When rerunning a task, show exactly what changed in the Context Manifest and why.

### Correction loop map

```text
Run 1 -> review rejected: scope drift
Run 2 -> verify failed: test X
Run 3 -> accepted
```

Show repeated failure causes rather than just run count.

### Knowledge debt

Detect repeated rediscovery:

- same command repeatedly looked up;
- same architectural rule repeatedly reintroduced manually;
- same context correction repeated by the human.

Suggest a knowledge candidate only when evidence repeats.

### Verification economy

Show whether expensive checks were rerun unnecessarily on unchanged relevant state.

### Change risk delta

Compare pre/post structural/security/architecture risk and attach the delta to the ChangeSet rather than a generic repository scan.

---

# 9. Differentiating feature brainstorm

These features strengthen the trustworthy-change thesis instead of widening RepoDesk into another IDE.

## 9.1 ChangeSet Passport

Every ChangeSet gets a compact immutable passport:

```text
ChangeSet ID
Work Item
baseline
producer run
isolation strength
files/hunks
scope status
review decision
verification receipts
acceptance coverage
commit
knowledge produced
```

This becomes the main object shared in review/debugging.

## 9.2 “Why?” inspector

For any important decision:

- Why this context source?
- Why excluded?
- Why this executor?
- Why was this file writable?
- Why is verification stale?
- Why is commit blocked?

Show deterministic evidence and policy chain.

## 9.3 Safe Commit Manifest

Before commit, render one deterministic summary:

```text
Work Item: W-123
ChangeSet: C-456
Scope: PASS
Protected paths: PASS
Human review: ACCEPTED
Required verification: 4/4 current
Unattributed changes: 0
Commit message: ...
```

Commit is a consequence of evidence, not a convenient button.

## 9.4 Verification replay

A receipt can be replayed against the current exact ChangeSet.

The product can explain:

- still current;
- stale because file X changed;
- tool unavailable;
- environment changed;
- result changed.

## 9.5 Scope drift map

Visualize files as:

- explicitly in scope;
- dependency-related but not explicitly allowed;
- protected;
- changed by accepted run;
- changed but unattributed.

This is more valuable than a generic diff stat.

## 9.6 Context provenance graph

Not a token pie chart.

Show:

```text
source -> selection reason -> bytes/tokens -> trust -> freshness -> used by run
```

Then compare run-to-run context changes.

## 9.7 Executor capability receipts

For each run, record actual enforcement:

- worktree isolation;
- writable roots;
- network policy;
- command policy;
- secret access;
- model/provider;
- user approval.

This prevents vague “sandboxed” claims.

## 9.8 Review handoff packet

Create a compact human review packet from a ChangeSet:

- intent;
- scope;
- risky files;
- architectural/security findings;
- important hunks;
- acceptance evidence gaps;
- verification results.

This attacks review fatigue directly.

## 9.9 Outcome-to-knowledge closure

At Finish, ask only evidence-backed questions:

- Did a project invariant become clearer?
- Did a command/check prove reusable?
- Did a known hazard change?
- Was an existing knowledge entry contradicted?

Knowledge candidates cite the exact Work Item/receipt/commit that motivated them.

## 9.10 Trust diff between two candidate agent runs

When two executors attempted the same Work Item, compare:

- scope adherence;
- files touched;
- verification;
- review findings;
- context size;
- cost;
- time;
- correction burden.

This is a defensible form of agent/model comparison because it measures engineering outcomes, not chatbot vibes.

---

# 10. What to delete before adding more

A deliberate deletion backlog:

1. separate Git route;
2. separate Orchestrate route;
3. Dashboard route;
4. Outcomes route;
5. Audit product route after canonical migration;
6. independent Models route;
7. independent Tokens route;
8. Models & Cost route after its data is redistributed;
9. System Registry route;
10. generic Playbooks route after Work Templates exist;
11. duplicate Related navigation in Work;
12. generic Related navigation in WorkspaceSidebar;
13. “brain” visible vocabulary;
14. old live JSONL audit writer;
15. silent legacy memory fallback on retrieval failure;
16. versioned CSS/class generations as their consumers migrate;
17. raw Git developer output from normal user flow;
18. duplicated mutable review/verification controls in Runs.

Deletion is a feature here. Every removed parallel surface lowers the cost of making the remaining five excellent.

---

# 11. Proposed large implementation cuts

These are intentionally larger coherent domain cuts rather than tiny cosmetic PRs.

## Cut A — Canonical evidence authority

### Scope

- remove live legacy audit JSONL write path;
- migrate/read historical audit data explicitly if needed;
- canonical event projection for technical audit view;
- eliminate user-facing split-brain evidence;
- make verification/ChangeSet/event IDs first-class across projections.

### Acceptance

- one authoritative event writer;
- UI audit/debug can be rebuilt from canonical data;
- historical JSONL is migration/export only;
- corruption/migration tests cover failure modes.

## Cut B — Context construction fail-closed

### Scope

- remove `.ok()` memory retrieval swallowing;
- explicit pinned overflow failure;
- explicit legacy fallback rule;
- Context Manifest records all source/fallback provenance;
- align Prepare and execution guard semantics.

### Acceptance

- Prepare cannot produce a “ready” context if required structured memory failed;
- same explicit inputs/clock produce replayable selection;
- every fallback is visible.

## Cut C — IA convergence

### Scope

- remove Git route -> Changes;
- remove Orchestrate route -> Work/Changes/Runs/Projects;
- remove Dashboard;
- remove Outcomes route -> Projects evidence;
- remove Models/Tokens/Models & Cost routes -> Projects/Runs;
- remove System route -> Projects/Settings;
- migrate Playbooks -> Work Templates;
- simplify sidebar/command palette/deep-link redirects.

### Acceptance

A normal user can perform all normal engineering work using only:

- Work;
- Code;
- Changes;
- Runs;
- Projects;
- Settings utility.

No hidden route is required for the normal lifecycle.

## Cut D — Changes as trust authority

### Scope

- Acceptance Evidence Matrix;
- exact ChangeSet passport;
- receipt staleness;
- provenance inspector;
- move mutable review/verification/commit decisions out of Runs;
- raw Git diagnostics demoted to technical detail.

### Acceptance

Changes alone can answer what changed, who/what produced it, whether it is allowed, whether it was reviewed, whether required evidence is current, and whether it can commit.

## Cut E — Projects as durable engineering configuration

### Scope

- move project setup/config from Settings;
- project checks/context rules/budgets;
- executors/capabilities;
- project knowledge management;
- Work Templates;
- routing evidence aggregates.

### Acceptance

Settings contains no project-specific engineering policy.

## Cut F — Design-system convergence

### Scope

- semantic status/evidence primitives;
- shared panel/empty/error/action/inspector primitives;
- remove raw status color maps;
- remove versioned visual classes;
- consolidate Work styles;
- migrate route styles as routes disappear;
- add CSS regression ratchet;
- visual regression snapshots for five primary surfaces.

### Acceptance

- no new raw status hex maps in feature code;
- no new `*-vN` visual generations;
- one status/evidence vocabulary;
- each primary surface has a clear style owner;
- deleting retired route CSS does not change unrelated surfaces.

## Cut G — Execution attribution + verification binding

### Scope

- formal attribution strength;
- isolated baseline for every supported write-capable executor where feasible;
- exact ChangeSet/tree identity;
- verification receipt state binding;
- safe commit manifest.

### Acceptance

RepoDesk never claims an exact producer or current verification without evidence sufficient to support the claim.

---

# 12. Recommended implementation order

## P0 — Trust correctness

1. Canonicalize the remaining audit JSONL writer into SQLite evidence.
2. Make Prepare context fail-closed and remove silent memory fallback-on-error.
3. Formalize ChangeSet attribution strength and exact state identity.
4. Bind/expire verification receipts against exact relevant state.
5. Fix application Quit -> dirty draft flush coordination.
6. Remove any remaining product claims that overstate sandboxing or heuristic complexity certainty.

## P1 — Product convergence

7. Remove Git as a route.
8. Dismantle Orchestrate as a route and redistribute responsibilities.
9. Make Changes sole mutable review/verify/commit authority.
10. Collapse Models/Tokens/Cost/Outcomes into Projects/Runs.
11. Merge System Registry into Projects/Settings.
12. Convert Playbooks into Work Templates.
13. Remove Dashboard and simplify WorkspaceSidebar/Work related navigation.

## P1 — Design convergence

14. Inventory status/panel/empty/error/action patterns.
15. Introduce semantic EvidenceState/StatusBadge and core layout primitives.
16. Remove inline status color maps/raw hex in feature code.
17. Remove versioned visual class generations.
18. Consolidate Work/Code/Changes CSS ownership.
19. Add CSS/design ratchet and five-route visual regression coverage.

## P2 — Differentiating product features

20. ChangeSet Passport.
21. Acceptance Evidence Matrix.
22. Why/decision inspector.
23. Verification replay/staleness UX.
24. Safe Commit Manifest.
25. Scope drift map.
26. Outcome-to-knowledge closure.
27. Trust comparison between multiple attempts/executors.
28. Structural Complexity Risk with compiler/LSP adapters.

## P2 — Release / validation

29. License/business decision.
30. updater signing canary;
31. macOS signing/notarization;
32. private vulnerability reporting;
33. Windows signing and branch protection;
34. explicit privacy/telemetry stance;
35. 90-second trustworthy-change demo;
36. 10–15 design partners before another broad feature wave.

---

# 13. Demo story after convergence

The product should be explainable without mentioning most implementation subsystems:

1. Connect a repository.
2. Create a Work Item.
3. Define goal, scope and acceptance criteria.
4. RepoDesk prepares a bounded Context Manifest and explains what it selected.
5. RepoDesk selects an eligible executor under explicit policy/cost constraints.
6. The executor works in the strongest supported isolation boundary.
7. RepoDesk captures the exact ChangeSet and provenance.
8. The developer reviews the ChangeSet.
9. RepoDesk maps verification receipts to acceptance criteria.
10. Commit becomes available only when the configured evidence contract is satisfied.
11. The completed change can produce reviewed reusable project knowledge.

The sales/product question becomes:

> **“Can you prove what your coding agents changed, why they were allowed to change it, what context they saw, and whether the exact accepted change was actually verified?”**

RepoDesk should answer yes.

---

# 14. Non-goals after this audit

Do not spend the next phase on:

- another AI chat;
- another dashboard;
- another provider/model catalog page;
- autonomous merge/push without human policy;
- a generic team collaboration suite;
- cloud token resale as the business model;
- “AI efficiency” magic scores;
- formal Big-O claims from syntax heuristics;
- a broad VS Code replacement;
- another CSS polish generation;
- another event store;
- hidden automatic adaptation that can bypass deterministic policy.

---

# 15. Definition of product convergence complete

The convergence phase is complete when all of the following are true:

- the normal lifecycle requires only Work, Code, Changes, Runs and Projects;
- Settings contains global settings, not project workflow policy;
- ChangeSet is the only user-facing abstraction that owns review/verification/commit readiness;
- Runs own execution evidence but not mutable change acceptance;
- SQLite is the sole live canonical engineering evidence ledger;
- Prepare and Execute enforce the same fail-closed context contract;
- write attribution explicitly states its evidence strength;
- verification receipts visibly bind to the exact state they prove;
- project knowledge is reviewed, attributable, lifecycle-bound and contextual;
- no “brain”, generic AI cockpit, standalone Git dashboard or orchestration super-page is required to explain the product;
- the design system has one semantic status/evidence vocabulary and no new versioned visual layers;
- the 90-second demo can be understood as one trustworthy software-change story.

At that point RepoDesk stops looking like several experiments accumulated in one desktop app and starts behaving like one product.