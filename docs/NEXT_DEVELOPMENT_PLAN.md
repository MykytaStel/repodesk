# RepoDesk Next Development Plan

## Product direction

RepoDesk is a **local-first, agent-agnostic engineering control and evidence workspace for trustworthy software change**.

It is not primarily an AI IDE, model launcher, or agent chat shell. Executors can change over time; the durable product value is the engineering contract around them:

`Goal → Scope → Context → Route → Execute → Review → Verify → Commit → Outcome → Knowledge`

The current product workflow remains:

`Scope → Prepare → Execute → Review → Verify → Finish`

Every phase should be derived from evidence rather than mutable UI state whenever possible.

## Current foundation

The repository already has working foundations that older roadmaps incorrectly described as future work:

- SQLite-backed state and migrations.
- Hash-chained engineering event journal.
- Bounded context construction and context provenance.
- Project memory and reviewed engineering knowledge.
- Deterministic routing with hard safety/budget constraints.
- Local, paid-provider, and coding-agent executor separation.
- Isolated worktree execution for write-capable coding agents.
- Exact changeset review and verification receipts.
- Security scanning, guarded command execution, CSP/capability restrictions, and keychain-backed credentials.
- Model-aware token/cost accounting.
- Desktop packaging, updater plumbing, backup/restore, CI, Playwright smoke tests, cargo-deny, and native Tauri E2E.

Do not create new roadmap phases for these capabilities. Improve their correctness and convergence instead.

---

# Milestone: Trustworthy Change Foundation

## Acceptance contract

A single Work Item must be able to produce one traceable evidence chain:

1. A user declares a goal and bounded scope.
2. RepoDesk constructs a bounded, attributable context manifest.
3. Policy and cost constraints determine which executors are eligible.
4. The selected executor operates in the narrowest supported isolation boundary.
5. RepoDesk captures the exact changeset produced by that execution.
6. Human review is bound to that changeset.
7. Verification is bound to the reviewed run/head/index/changeset.
8. Finish requires a real commit, not a UI acknowledgement.
9. Outcome and durable knowledge can be traced back to the Work Item and evidence that produced them.

If RepoDesk cannot prove an invariant required for the next phase, the next phase must be blocked rather than inferred optimistically.

---

## P0 — State integrity and recovery

### Objective

User state must survive malformed backups, interrupted restores, process failure, and application upgrades without silent corruption.

### Required work

- Keep restore validation and rollback fail-closed.
- Prefer SQLite-safe snapshot/backup semantics over copying a live database when further backup work is added.
- Exercise restore against valid older schemas, unrelated SQLite files, corrupt files, and interrupted replacement scenarios.
- Surface recovery failures explicitly to the desktop boundary.
- Define recovery evidence so a restored database can report what source was used and whether migrations/integrity checks passed.

### Done when

- A rejected restore never mutates the live database.
- A failed replacement can recover the pre-restore database.
- Restore/migration/integrity failures are test-covered and observable.

---

## P1 — One canonical engineering evidence ledger

### Objective

Eliminate split-brain histories. SQLite is the sole canonical engineering event store; JSONL is export or read-only migration compatibility.

### Required work

- Route typed engineering instrumentation through the canonical hash-chained SQLite journal.
- Preserve historical task-local JSONL without continuing to append to it.
- Keep Work Item, execution, changeset, verification, commit, outcome, and knowledge identities queryable as first-class metadata.
- Build Engineering Intelligence projections from canonical events rather than a second event stream.
- Add migration/deduplication rules for historical evidence.
- Preserve fail-closed hash/sequence verification on reads and writes.

### Done when

There is exactly one authoritative write path for engineering events and every user-facing projection can be rebuilt from canonical state plus explicit legacy migration inputs.

---

## P1 — Bounded and replayable context

### Objective

The same inputs and explicit clock produce the same context decision, and context limits are hard limits.

### Required work

- Keep memory ranking replayable with an explicit clock.
- Keep the final rendered context under the configured hard token ceiling.
- Treat omitted pinned constraints as a blocker, not ordinary truncation.
- Remove best-effort `.ok()` fallbacks from Prepare where they can hide memory/provenance failures.
- Give pinned constraints a documented reserved budget or return an explicit construction failure when they cannot fit.
- Record a Context Manifest with fingerprints, provenance, trust, relevance/freshness inputs, inclusion/exclusion decisions, and token accounting.
- Later add hybrid retrieval: lexical + semantic + provenance confidence + scope + recency + contradiction state + diversity.

### Done when

An execution can explain exactly what context it saw, why each component was included, what was excluded, what time-dependent inputs were used, and whether any required constraint failed to fit.

---

## P1 — Security and execution boundaries

### Objective

Security policy must be precise enough to block dangerous behavior without blocking ordinary source code by vocabulary accident.

### Required work

- Maintain one canonical path-policy implementation based on path segments, basenames, extensions, and explicit patterns — not arbitrary substrings.
- Keep remote custom providers HTTPS-only; allow plaintext HTTP only for exact loopback hosts.
- Reject embedded URL credentials and bound/redact third-party error bodies.
- Continue separating worktree isolation, command policy, agent-native sandboxing, filesystem permissions, and network permissions in code and product language.
- Centralize execution-policy decisions so command checks and file access cannot drift into separate definitions.
- Add explicit network/filesystem capability receipts where executors support them.

### Done when

Every execution can state what isolation and permissions were actually enforced rather than calling all controls a generic “sandbox”.

---

## P1 — Routing and cost contracts

### Objective

Routing must be deterministic, constraint-first, inspectable, and financially truthful.

### Required work

- Treat hard limits such as `max_cost_units`, unsafe context, token ceilings, unavailable auth, and required executor capabilities as blockers before scoring.
- Replace stringly risk interpretation with typed risk/constraint data.
- Separate routing into:
  1. hard constraints,
  2. normalized objectives,
  3. learned residual/bias that can never bypass hard constraints.
- Move model selection from “preferred/first model” toward model-level capability, price, latency, and observed-outcome ranking.
- Keep cost rates keyed by provider + model with explicit fallback semantics.
- Version pricing provenance so historical estimates do not silently change when the current rate card changes.
- Distinguish estimated and actual cost wherever providers expose actual usage.

### Done when

RepoDesk can explain both why the winning route was eligible and why it beat every other eligible route.

---

## P1 — Editor and local-workspace correctness

### Objective

The Code surface must never confuse display paths with document identity or lose unsaved user work during application shutdown.

### Required work

- Replace path-only tab identity with stable IDs, for example:
  - `workspace:<project-id>:<path>`
  - `library:<document-handle>`
- Use tab identity for open, activate, close, eviction, save, rename, delete, and cache restoration.
- Ensure save only updates the matching workspace document.
- Keep repository findings/intelligence scoped to an active workspace file, never a same-path library document.
- Add native quit dirty-state/draft flush coordination so edit → immediate Quit cannot outrun the debounce timer.
- Add regression coverage for workspace/library path collisions and duplicate library display paths.

### Done when

Two documents may share the same visible path without sharing state, and a dirty edit survives every supported close/quit path.

---

## P1 — UX reliability

### Objective

A failure in an optional or lazy-loaded surface must not take down the whole engineering workspace.

### Required work

- Add feature-local error boundaries and retry for lazy routes, Command Palette, bottom panel, Terminal, and IDE Health where appropriate.
- Preserve the five primary product surfaces:
  - Work
  - Code
  - Changes
  - Runs
  - Projects
- Make Work Item the navigation spine for secondary information.
- Show current phase, blocking invariant, context/security/cost status, selected route, baseline, and next safe action together.
- Prefer explicit blocked/unknown states over optimistic empty states.

### Done when

The user can recover an optional feature without losing the active Work Item or restarting the whole application.

---

## P2 — Product and information-architecture convergence

### Objective

Reduce duplicate concepts rather than adding more panels.

### Required work

Consolidate or demote legacy overlaps such as:

- Changes vs Git.
- Runs vs Outcomes vs Audit vs Debug.
- Models & Cost vs Models vs Tokens.
- Knowledge vs legacy Memory Brain.
- Orchestrate surfaces that duplicate the primary Work Item flow.

Keep deep links where migration requires them, but do not expose legacy concepts as equal top-level product destinations.

### Done when

A new user can understand the product through the five primary surfaces and inspect advanced evidence contextually instead of learning multiple parallel navigation models.

---

## P2 — Design-system convergence

### Objective

Stop accumulating versioned CSS polish layers.

### Required work

Converge toward:

1. tokens/foundation,
2. reusable primitives,
3. application shell,
4. workbench components,
5. feature-local styles.

Remove legacy cascade ownership as consumers migrate. Do not add another `polish-vN` layer to solve local styling debt.

### Done when

A component's visual ownership is obvious from its primitive/workbench/feature layer and removing a historical polish file does not unpredictably restyle unrelated screens.

---

## P2 — Architecture boundaries

### Objective

Prevent `repodesk-core` from becoming an unbounded god crate while avoiding a big-bang rewrite.

### Required work

First enforce internal module dependency direction; split physical crates only when boundaries are stable.

Target bounded contexts:

- Domain/contracts.
- Ledger/evidence.
- Workspace/Git/change management.
- Execution/policy/providers.
- Intelligence/context/knowledge/routing.

Potential future crates:

- `repodesk-domain`
- `repodesk-ledger`
- `repodesk-workspace`
- `repodesk-exec`
- `repodesk-intelligence`

### Done when

Cross-context dependencies are explicit, dependency direction is enforceable, and large modules can be moved without changing product behavior.

---

## P2 — Engineering Intelligence

### Objective

Turn deterministic evidence into useful engineering judgment without overstating certainty.

### Algorithmic profile

Rename/position heuristic Big-O output as **Structural Complexity Risk** unless semantic evidence is available.

Show:

- detected structure,
- likely bound,
- plausible alternatives,
- confidence,
- missing semantic evidence.

Add semantic adapters incrementally:

- Rust: rust-analyzer/HIR/type information.
- TypeScript: compiler/tsserver AST + types.

### Knowledge

Durable knowledge must carry provenance and lifecycle state: proposed → reviewed → accepted/rejected → archived/superseded.

### Trust Graph

Converge toward this queryable relationship model:

`WorkItem → Goal → Scope → ContextManifest → RoutingDecision → ExecutorRun → Changeset → ReviewDecision → VerificationReceipt → Commit → Outcome → Knowledge`

Every node must have stable identity and provenance.

Useful questions RepoDesk should eventually answer directly:

- Why was this file changed?
- What context did this executor see?
- Why was this executor/model selected?
- Were these verification results produced for this exact changeset?
- Which human reviewed it?
- Which accepted knowledge was learned from the outcome?

---

## Release readiness

Public distribution is not “done” because an application binary can be built.

### P0 release decisions

- Choose and add a LICENSE consistent with the intended business model.
- Configure updater signing secrets and validate a real signed updater canary.
- Configure macOS signing and notarization.
- Enable GitHub private vulnerability reporting.

### P1 release hardening

- Windows code signing.
- Branch protection / required CI checks for `main`.
- Legal/privacy review for cloud-provider integrations.
- Explicit telemetry stance.

### P2 supply-chain evidence

- Pin GitHub Actions to immutable commit SHAs where practical.
- Generate an SBOM for release artifacts.
- Keep cargo-deny/gitleaks/secret scanning as required gates.
- Align crate/app versioning before declaring a stable public 1.0 contract.

---

## Product / business validation

### Initial ICP

Prioritize senior/staff developers and small engineering teams that already use multiple coding agents/models and feel pain from:

- scope drift,
- opaque agent context,
- unsafe or unreviewable changes,
- duplicated work,
- uncertain AI spend,
- review fatigue,
- project knowledge loss.

### Product promise

RepoDesk should make a software change more **bounded, attributable, reviewable, verifiable, and recoverable**, regardless of which executor produced it.

### Demo path

The core product demo should stay approximately 90 seconds:

`Connect repo → Create Work Item → Bound Context → Select/justify executor → Run → Review exact changeset → Verify → Commit → Capture outcome/knowledge`

### Validation before feature expansion

- Recruit 10–15 design partners from the ICP.
- Measure which evidence/guardrail steps users actually inspect or override.
- Test willingness to pay for team governance rather than token resale.
- Build a clear “Why RepoDesk if I already use Cursor/Codex/etc.?” comparison around evidence and control, not model quality.

---

## Do not do yet

Avoid:

- Competing on “our agent writes better code”.
- Adding another top-level AI chat surface.
- Unbounded repository dumps to cloud models.
- Automatic patch acceptance without exact changeset review and verification binding.
- Learned routing that can bypass hard policy constraints.
- New state stores or event streams that duplicate canonical SQLite evidence.
- New design “polish layers” instead of consolidating the existing design system.
- Large physical crate splits before dependency boundaries are proven.
- Broad feature waves while P0/P1 correctness invariants are still open.

## Definition of the next phase

The next feature wave starts only when the Trustworthy Change Foundation is demonstrably true for the normal Work Item path and its failure modes.
