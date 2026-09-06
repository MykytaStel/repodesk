# RepoDesk AI Work Control Plane Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the first usable RepoDesk control-plane slice: truthful run economics, explainable verification recommendations, Decision Receipts, and a focused Work Item UI that helps a developer decide what to run next and when to stop.

**Architecture:** Extend the existing deterministic Engineering event ledger and projection model with a typed verification recommendation and Decision Receipt. Expose the read/write contract through a small Tauri command surface and render it in a redesigned Work Item control view. Phase 1 is recommendation-first: automatic test skipping, automatic model rerouting, and loop intervention remain behind later policies.

**Tech Stack:** Rust workspace (`repodesk-core`), Tauri 2 command layer, React/TypeScript, TanStack Query, existing CSS cascade and design tokens, Playwright browser E2E, Rust unit tests, TypeScript build checks.

**Spec:** `docs/superpowers/specs/2026-09-07-ai-work-control-plane-design.md`

## Global Constraints

- RepoPilot remains a deterministic `ChangeProof` provider; RepoDesk must not reimplement or contradict its verdicts.
- Deterministic facts precede model judgment.
- Critical project checks cannot be silently skipped.
- Unknown is a valid result; missing evidence is not proof of safety.
- Every verification receipt is bound to an exact tree/worktree identity.
- A skipped check creates visible verification debt.
- Phase 1 is recommendation-first; automatic policy actions require explicit project policy.
- The implementation agent cannot be the sole authority that declares its own work complete.
- Raw prompts and full model responses are not required for the first analytics version.
- Local-first behavior and existing credential/redaction boundaries remain unchanged.
- The primary UI remains organized around Work, Code, Changes, Runs, and Projects; Tokens, Audit, Models, and Dashboard do not become competing primary destinations.
- Do not add a new provider, agent, database, or frontend state-management library for this slice.

## Scope boundary

This plan deliberately covers one coherent, testable Phase 0/1 slice. It does
not implement Phase 2 loop intervention, report-only transitions, grouped test
failures, or Phase 3 project learning. Those need separate follow-up plans
after the first slice produces real evidence.

## File map before implementation

### Domain and projection

- Create `crates/repodesk-core/src/engineering/decision_receipt.rs` — typed receipt, recommendation actions, verification debt, and serialization boundaries.
- Create `crates/repodesk-core/src/engineering/adaptive_verification.rs` — deterministic recommendation policy and input/output types.
- Modify `crates/repodesk-core/src/engineering/events.rs` — Phase 0/1 event kinds and stable labels.
- Modify `crates/repodesk-core/src/engineering/mod.rs` — module declarations and public exports.
- Modify `crates/repodesk-core/src/engineering/intelligence.rs` — accepted-change economics read model inputs.
- Modify `crates/repodesk-core/src/engineering/ai_usage_intelligence.rs` — expose outcome metrics needed by the control view without creating a synthetic score.
- Modify `crates/repodesk-core/src/engineering/instrumentation.rs` — shared event helpers for recommendations, selections, deferrals, and overrides.
- Modify `crates/repodesk-core/src/checks.rs` and `crates/repodesk-core/src/checks/execution.rs` — check candidates and tree-bound verification receipt facts.
- Modify `crates/repodesk-core/src/workflow/receipt.rs` — preserve/extend receipt freshness and exact-tree binding where the existing workflow owns it.

### Desktop command and API boundary

- Create `apps/desktop/src-tauri/src/commands/verification.rs` — Tauri DTOs and commands for the advisor snapshot and explicit decision recording.
- Modify `apps/desktop/src-tauri/src/commands/mod.rs` — command module export.
- Modify `apps/desktop/src-tauri/src/lib.rs` — register the new commands and keep the command inventory testable.
- Modify `apps/desktop/src-tauri/src/commands/workflow.rs` and `apps/desktop/src-tauri/src/commands/orchestrate.rs` — call the policy/read model at existing preflight and verification boundaries rather than creating a second workflow.
- Create `apps/desktop/src/shared/api/verification.ts` — typed IPC client and query keys.
- Modify `apps/desktop/src/shared/api/queries.ts` — add the verification query domain.
- Modify `apps/desktop/src/shared/api/cacheInvalidation.ts` — invalidate verification and run projections after execution, review, and verify mutations.
- Modify `apps/desktop/src/shared/types/api.ts` only if a new `TabId` or shared DTO is genuinely required; prefer feature-local types.

### Work Item UI/UX

- Create `apps/desktop/src/features/work/RunControlCard.tsx` — current run state, spend, budget, iteration/attempt facts, and pause/stop/review actions where already supported.
- Create `apps/desktop/src/features/work/VerificationAdvisorCard.tsx` — recommendation-first check decision surface with selected/deferred actions and reasons.
- Create `apps/desktop/src/features/work/DecisionReceiptDrawer.tsx` — inspectable facts, evidence, policy, override, and verification debt.
- Create `apps/desktop/src/features/work/ChangeEconomicsCard.tsx` — cost per accepted change and bounded efficiency metrics with “unknown” states.
- Create `apps/desktop/src/features/work/work-control.css` — the new Work control visual language, imported through the existing Work feature stylesheet and existing cascade layers.
- Modify `apps/desktop/src/features/work/WorkSurface.tsx` — compose the new Work Item control layout and keep Contract/Context/Intelligence as secondary inspectors.
- Modify `apps/desktop/src/features/work/WorkTab.tsx` — replace the current phase-heavy composition with the next-action control view while preserving existing workflow mutations.
- Modify `apps/desktop/src/features/work/WorkIntelligenceCard.tsx` — render evidence links and drill-down metrics instead of competing with the primary action.
- Modify `apps/desktop/src/features/work/work-route.css` — remove conflicting card/grid rules made obsolete by the control view.
- Modify `apps/desktop/src/app/ActivityRail.tsx`, `apps/desktop/src/app/WorkspaceSidebar.tsx`, and `apps/desktop/src/app/tabs.tsx` — improve navigation labels and contextual hierarchy without adding routes.
- Modify `apps/desktop/src/app/styles/visual-language-2026.css` only for shared tokens required by the new control surface; do not append another versioned override layer.

### Verification

- Create `crates/repodesk-core/src/engineering/adaptive_verification_tests.rs` only if inline tests make the policy module unreadable; otherwise keep unit tests next to the module.
- Modify `apps/desktop/e2e/work-golden-path.spec.ts` — extend the existing golden path with preflight/advisor/receipt assertions.
- Create `apps/desktop/e2e/adaptive-verification.spec.ts` — browser-only command mocks for recommendation, deferral, and receipt inspection.
- Create `apps/desktop/e2e/run-control.spec.ts` — budget, spend, stale/unknown evidence, and accessible control states.
- Modify `apps/desktop/e2e/work-design-system.spec.ts` and `apps/desktop/e2e/ui-audit.spec.ts` — assert the new hierarchy, no duplicate primary actions, and key accessibility contracts.

---

### Task 1: Define typed verification decisions and Decision Receipts

**Files:**
- Create: `crates/repodesk-core/src/engineering/decision_receipt.rs`
- Modify: `crates/repodesk-core/src/engineering/mod.rs`
- Modify: `crates/repodesk-core/src/engineering/events.rs`

**Interfaces:**

Produce these stable Rust types for later tasks:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationDecisionKind {
    RunNow,
    RunTargeted,
    DeferWithDebt,
    AskForApproval,
    PauseAndReview,
    StopWithPartialResult,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationDebt {
    pub check_id: String,
    pub title: String,
    pub reason: String,
    pub required_before: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionReceipt {
    pub decision_id: String,
    pub project: String,
    pub work_item_id: String,
    pub execution_id: Option<String>,
    pub tree_identity: String,
    pub changeset_identity: Option<String>,
    pub decision_kind: VerificationDecisionKind,
    pub policy_version: String,
    pub observed_facts: BTreeMap<String, serde_json::Value>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub selected_actions: Vec<String>,
    pub skipped_actions: Vec<String>,
    pub estimated_cost_units: Option<f64>,
    pub estimated_wall_clock_ms: Option<u64>,
    pub risk_label: String,
    pub uncertainty_label: String,
    pub verification_debt: Vec<VerificationDebt>,
    pub human_override: Option<String>,
    pub outcome: Option<String>,
    pub created_at: DateTime<Utc>,
}
```

`EvidenceRef` must reuse the existing engineering-domain type. Do not store
raw prompt or response bodies in the receipt.

- [ ] **Step 1: Add failing serialization and invariant tests.**

Add tests for:

```rust
#[test]
fn decision_kind_serializes_as_snake_case() { /* run_targeted */ }

#[test]
fn receipt_preserves_unknown_and_deferred_evidence() { /* no inferred green */ }

#[test]
fn receipt_requires_tree_identity_and_policy_version() { /* reject empty values */ }
```

Expected: the new tests fail because the types and validation do not exist.

- [ ] **Step 2: Run the focused Rust tests and confirm failure.**

Run:

```bash
cargo test -p repodesk-core decision_receipt -- --nocapture
```

Expected: compile/test failure naming the missing types or validation.

- [ ] **Step 3: Implement the types, constructors, and validation.**

Use a constructor that rejects blank `tree_identity`, `project`,
`work_item_id`, and `policy_version`. Keep optional cost/identity fields
optional so unavailable data remains explicit rather than fabricated.

Extend `EngineeringEventKind` only with the Phase 0/1 values:

```text
PreflightEstimated
VerificationRecommended
VerificationSelected
VerificationDeferred
DecisionOverridden
DecisionOutcomeRecorded
```

Add stable snake-case labels and public exports from `engineering/mod.rs`.

- [ ] **Step 4: Run the focused tests and formatting.**

Run:

```bash
cargo test -p repodesk-core decision_receipt -- --nocapture
cargo fmt --all -- --check
```

Expected: PASS with no formatting diff.

- [ ] **Step 5: Commit the domain contract.**

```bash
git add crates/repodesk-core/src/engineering/decision_receipt.rs \
  crates/repodesk-core/src/engineering/events.rs \
  crates/repodesk-core/src/engineering/mod.rs
git commit -m "feat: add verification decision receipts"
```

### Task 2: Build the deterministic Adaptive Verification policy

**Files:**
- Create: `crates/repodesk-core/src/engineering/adaptive_verification.rs`
- Modify: `crates/repodesk-core/src/engineering/mod.rs`
- Modify: `crates/repodesk-core/src/checks.rs`
- Modify: `crates/repodesk-core/src/checks/execution.rs`

**Interfaces:**

The policy must be pure and deterministic for the same input and policy
version. Define these types:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationCheckCandidate {
    pub id: String,
    pub title: String,
    pub command: String,
    pub kind: String,
    pub required: bool,
    pub estimated_seconds: Option<u64>,
    pub estimated_cost_units: Option<f64>,
    pub relevant_paths: Vec<String>,
    pub last_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationAdvisorInput {
    pub project: String,
    pub work_item_id: String,
    pub tree_identity: String,
    pub changed_files: Vec<String>,
    pub risk_label: String,
    pub proof_obligations: Vec<String>,
    pub checks: Vec<VerificationCheckCandidate>,
    pub estimated_budget_units: Option<f64>,
    pub prior_attempts: usize,
    pub prior_failures: usize,
    pub policy_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationRecommendation {
    pub decision: VerificationDecisionKind,
    pub rationale: Vec<String>,
    pub selected_check_ids: Vec<String>,
    pub deferred_checks: Vec<VerificationDebt>,
    pub estimated_cost_units: Option<f64>,
    pub estimated_wall_clock_ms: Option<u64>,
    pub risk_label: String,
    pub uncertainty_label: String,
    pub policy_version: String,
}

pub fn recommend_verification(
    input: &VerificationAdvisorInput,
) -> VerificationRecommendation;
```

Required policy behavior:

- select required checks whenever they are available;
- prefer checks whose relevant paths intersect the change;
- prefer cheaper/shorter checks before broader checks when they provide a
  meaningful signal;
- return `AskForApproval` when a non-required full suite is expensive and the
  evidence is insufficient to defer it safely;
- return `DeferWithDebt` only with explicit debt entries;
- return `StopWithPartialResult` when prior failures indicate no progress and
  the input policy allows stopping;
- never claim a probability or confidence percentage unless it comes from
  recorded project evidence; use uncertainty labels instead.

- [ ] **Step 1: Add policy tests covering the decision matrix.**

Test at least:

```rust
#[test]
fn required_check_is_selected_even_when_expensive() { /* run_now */ }

#[test]
fn focused_change_prefers_intersecting_targeted_check() { /* run_targeted */ }

#[test]
fn expensive_unrelated_suite_becomes_explicit_debt() { /* defer_with_debt */ }

#[test]
fn repeated_failed_attempts_request_human_review() { /* ask_for_approval */ }

#[test]
fn missing_history_returns_unknown_not_false_confidence() { /* conservative */ }

#[test]
fn same_input_and_policy_produce_same_recommendation() { /* deterministic */ }
```

- [ ] **Step 2: Run the policy tests and confirm they fail.**

```bash
cargo test -p repodesk-core adaptive_verification -- --nocapture
```

Expected: failure until the policy types and function exist.

- [ ] **Step 3: Implement the minimal deterministic policy.**

Keep ranking rules in one function/module. Do not call an LLM, inspect raw
transcripts, or mutate project settings from the policy. Use existing check
definitions and cost/budget types where compatible; add adapters rather than
duplicating cost configuration.

- [ ] **Step 4: Add check candidate facts and exact-tree receipt inputs.**

Extend check execution results with the minimum factual fields needed by the
advisor and receipt:

```text
check_id
command
tree_identity
started_at
finished_at
exit_code
status
tests_observed (optional)
log_evidence_ref
```

When a runner cannot provide `tests_observed`, return `None`, not zero.

- [ ] **Step 5: Run focused tests and clippy.**

```bash
cargo test -p repodesk-core adaptive_verification checks -- --nocapture
cargo clippy -p repodesk-core --all-targets --all-features -- -D warnings
```

Expected: PASS with no warnings.

- [ ] **Step 6: Commit the policy slice.**

```bash
git add crates/repodesk-core/src/engineering/adaptive_verification.rs \
  crates/repodesk-core/src/engineering/mod.rs \
  crates/repodesk-core/src/checks.rs \
  crates/repodesk-core/src/checks/execution.rs
git commit -m "feat: add adaptive verification recommendations"
```

### Task 3: Persist receipts and expose a focused desktop IPC contract

**Files:**
- Modify: `crates/repodesk-core/src/engineering/instrumentation.rs`
- Modify: `crates/repodesk-core/src/engineering/intelligence.rs`
- Modify: `crates/repodesk-core/src/engineering/ai_usage_intelligence.rs`
- Create: `apps/desktop/src-tauri/src/commands/verification.rs`
- Modify: `apps/desktop/src-tauri/src/commands/mod.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/src/commands/workflow.rs`
- Modify: `apps/desktop/src-tauri/src/commands/orchestrate.rs`
- Create: `apps/desktop/src/shared/api/verification.ts`
- Modify: `apps/desktop/src/shared/api/queries.ts`
- Modify: `apps/desktop/src/shared/api/cacheInvalidation.ts`

**Interfaces:**

Expose only the operations needed by the first Work Item control view:

```rust
#[tauri::command]
pub fn work_verification_advisor() -> Result<VerificationAdvisorSnapshot, ErrorPayload>;

#[tauri::command]
pub fn work_record_decision(
    input: RecordDecisionInput,
) -> Result<DecisionReceiptDto, ErrorPayload>;

#[tauri::command]
pub fn work_decision_receipt(
    decision_id: Option<String>,
) -> Result<Option<DecisionReceiptDto>, ErrorPayload>;
```

`work_record_decision` may record a selection, deferral, or explicit human
override. It must not silently execute a shell command. Existing workflow
commands remain the owners of execution.

The TypeScript API must mirror the DTOs with discriminated unions and call the
existing `callCommand` wrapper so errors and runtime timing remain observable:

```ts
export type VerificationDecisionKind =
  | "run_now"
  | "run_targeted"
  | "defer_with_debt"
  | "ask_for_approval"
  | "pause_and_review"
  | "stop_with_partial_result";

export async function workVerificationAdvisor(): Promise<VerificationAdvisorSnapshot>;
export async function recordWorkDecision(input: RecordDecisionInput): Promise<DecisionReceipt>;
export async function getDecisionReceipt(decisionId?: string): Promise<DecisionReceipt | null>;
```

- [ ] **Step 1: Add Rust command DTO tests before wiring commands.**

Test that:

- blank decision IDs and tree identities are rejected;
- a deferred check must carry a debt reason;
- a decision receipt round-trips through the command DTO;
- recording a decision appends an event instead of mutating the historical
  event stream;
- missing cost, test count, or RepoPilot evidence stays `None`/unknown.

- [ ] **Step 2: Run the command/module tests and confirm failure.**

```bash
cargo test -p repodesk-desktop verification -- --nocapture
```

Expected: failure until the command DTOs and persistence adapter exist.

- [ ] **Step 3: Add typed event instrumentation and projection fields.**

Use the existing `append_event`/SQLite-backed ledger path. Add event helpers
that attach the Work Item, execution, worker, changeset, verification, and
evidence references already available at the call site. Extend the existing
AI usage projection with:

```text
accepted_change_count
accepted_change_cost_units
correction_cost_units
verification_debt_count
decision_count
override_count
```

Do not add an opaque productivity score or a second analytics store.

- [ ] **Step 4: Wire the read command to existing workflow state.**

The advisor snapshot must derive from the active Work Item, current Git/change
state, available check catalog, current budget, and existing RepoPilot findings
when available. If any input is unavailable, return a visible unknown reason.

- [ ] **Step 5: Wire explicit record/receipt commands and cache invalidation.**

Register commands in `apps/desktop/src-tauri/src/lib.rs`, add query keys, and
invalidate `work`, `runs`, `git`, and `verification` after a recorded decision
or verification mutation. Keep command registration tests updated.

- [ ] **Step 6: Add TypeScript API tests or compile fixtures.**

Use the existing frontend test conventions. Verify snake-case IPC fields are
mapped to the local types and that unknown optional values render without
coercing them to zero or “passed”.

- [ ] **Step 7: Run the complete backend gate for this slice.**

```bash
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: PASS before any UI changes are merged on top.

- [ ] **Step 8: Commit the IPC and projection slice.**

```bash
git add crates/repodesk-core/src/engineering \
  crates/repodesk-core/src/checks \
  apps/desktop/src-tauri/src/commands \
  apps/desktop/src-tauri/src/lib.rs \
  apps/desktop/src/shared/api/verification.ts \
  apps/desktop/src/shared/api/queries.ts \
  apps/desktop/src/shared/api/cacheInvalidation.ts
git commit -m "feat: expose verification control evidence"
```

### Task 4: Redesign the Work Item control surface

**Files:**
- Create: `apps/desktop/src/features/work/RunControlCard.tsx`
- Create: `apps/desktop/src/features/work/VerificationAdvisorCard.tsx`
- Create: `apps/desktop/src/features/work/DecisionReceiptDrawer.tsx`
- Create: `apps/desktop/src/features/work/ChangeEconomicsCard.tsx`
- Create: `apps/desktop/src/features/work/work-control.css`
- Modify: `apps/desktop/src/features/work/WorkSurface.tsx`
- Modify: `apps/desktop/src/features/work/WorkTab.tsx`
- Modify: `apps/desktop/src/features/work/WorkIntelligenceCard.tsx`
- Modify: `apps/desktop/src/features/work/work-route.css`

**UX contract:**

The Work surface must make the next decision obvious within one viewport. It
must not present a wall of equal-weight cards or make the user reconstruct the
state from phase labels.

Layout:

```text
Work Item header: goal | project | current status | spend | next action

Primary column:       Secondary column:
  Next action            Run Control
  Verification Advisor   Cost / accepted change
  Evidence status         Contract / Context / Intelligence inspector links

Bottom/inspector: Decision Receipt, verification debt, raw evidence
```

Required copy and states:

- `What should happen next?` is the primary section heading.
- The main CTA describes the action and cost, e.g. `Run targeted checks · ~2 min`.
- A skipped check is shown as `Deferred` with its debt reason, never hidden.
- Missing data is labeled `Unknown` or `Not measured`, never `0` or `Passed`.
- The user can open the Decision Receipt without leaving the Work Item.
- Existing workflow phases remain available as secondary context, not the
  primary mental model.

Visual rules:

- use the existing CSS cascade and tokens; add no new `*-v2`/`*-v3` override;
- one visually dominant action per state;
- replace stacked rounded cards with two clear regions and lightweight
  evidence rows;
- reserve accent color for the current action and warning/critical colors for
  actual evidence states;
- keep motion optional and preserve high-contrast/reduced-motion behavior;
- every icon-only control has an accessible name and visible tooltip/title;
- every status badge has text, not color alone.

- [ ] **Step 1: Add component contract tests/fixtures.**

Create deterministic fixture objects for:

- no project/no Work Item;
- ready to run with a targeted recommendation;
- expensive full suite deferred with debt;
- unknown cost/evidence;
- blocked or stale verification.

The fixtures must be plain TypeScript objects so the visual components can be
tested without invoking Tauri.

- [ ] **Step 2: Run the existing Work E2E tests to establish the baseline.**

```bash
pnpm --dir apps/desktop exec playwright test e2e/work-golden-path.spec.ts e2e/work-design-system.spec.ts
```

Record the baseline failures/snapshots before changing selectors or layout.

- [ ] **Step 3: Implement `RunControlCard`.**

Render current execution state, provider/model, token/cost facts, budget state,
attempt count, and the existing safe mutations. Use `Unknown` for missing
values. Keep pause/stop controls disabled unless an existing command supports
the action; do not fake runtime control in the UI.

- [ ] **Step 4: Implement `VerificationAdvisorCard`.**

Render the recommendation, rationale facts, selected checks, deferred checks,
estimated time/cost, and explicit buttons for `Accept recommendation`,
`Defer with debt`, and `Inspect evidence`. Recording a choice creates a
Decision Receipt but does not run a check unless the existing workflow action
is explicitly selected.

- [ ] **Step 5: Implement `DecisionReceiptDrawer`.**

Show tree identity, policy version, observed facts, evidence links, selected
and skipped actions, override reason, actual outcome, and verification debt.
Use progressive disclosure for long evidence lists and never render raw
prompts by default.

- [ ] **Step 6: Implement `ChangeEconomicsCard`.**

Show cost per accepted change when available, otherwise a clear unavailable
state. Include total spend, correction/retry cost, verification cost, and the
evidence timestamp. Do not render a single “AI productivity score”.

- [ ] **Step 7: Compose the new Work surface.**

Update `WorkSurface.tsx` and `WorkTab.tsx` so the new cards own primary
hierarchy. Keep Contract, Context, and Intelligence in the existing inspector
drawer. Remove duplicate phase summaries and any second primary CTA that asks
the user to choose between equivalent actions.

- [ ] **Step 8: Add the focused CSS and remove obsolete Work overrides.**

Place new rules in `work-control.css`, imported from the existing Work route
stylesheet. Use semantic classes for state (`is-warning`, `is-blocked`,
`is-unknown`) and test light, dark, high-contrast, narrow, and reduced-motion
states.

- [ ] **Step 9: Run UI typecheck/build.**

```bash
pnpm --dir apps/desktop build
```

Expected: TypeScript, performance budget, Vite build, and entry-budget checks
all pass.

- [ ] **Step 10: Commit the Work control redesign.**

```bash
git add apps/desktop/src/features/work
git commit -m "feat: add work item control surface"
```

### Task 5: Converge navigation and secondary surfaces around the control loop

**Files:**
- Modify: `apps/desktop/src/app/tabs.tsx`
- Modify: `apps/desktop/src/app/ActivityRail.tsx`
- Modify: `apps/desktop/src/app/WorkspaceSidebar.tsx`
- Modify: `apps/desktop/src/features/history/HistoryTab.tsx`
- Modify: `apps/desktop/src/features/history/RunsWorkspace.tsx`
- Modify: `apps/desktop/src/features/outcomes/OutcomesTab.tsx`
- Modify: `apps/desktop/src/features/audit/AuditTab.tsx`
- Modify: `apps/desktop/src/features/models-cost/ModelsCostTab.tsx`
- Modify: `apps/desktop/src/app/styles/visual-language-2026.css`

**UX contract:**

- primary navigation remains Work, Code, Changes, Runs, Projects;
- Runs owns execution evidence, Decision Receipts, and Change Economics;
- Audit becomes a deep evidence view, not a parallel product interpretation;
- Outcomes becomes a Change Economics subview, not a provider leaderboard;
- Models/Cost remains available from routing/settings or the relevant Run
  inspector, but does not compete with Work as a top-level destination;
- navigation labels explain the engineering job, not the internal subsystem.

- [ ] **Step 1: Add navigation contract assertions before changes.**

Extend the existing design-system E2E tests to assert:

- exactly five primary engineering destinations;
- no primary “Tokens”, “Audit”, “Models”, or generic “Dashboard” destination;
- active Work Item context remains visible when moving between Work and Runs;
- keyboard navigation and accessible labels remain intact.

- [ ] **Step 2: Update the rail and sidebar hierarchy.**

Make the rail labels discoverable through accessible names and the sidebar
describe current Work Item state, spend, changes, and next action. Avoid
adding another persistent status strip that duplicates the Work control header.

- [ ] **Step 3: Recompose Runs and Outcomes.**

Change `HistoryTab` subviews to:

```text
Run timeline | Change economics | Evidence archive
```

Keep the raw audit trail available inside Evidence archive/inspector, but do
not present it as the default user journey.

- [ ] **Step 4: Remove duplicate models/tokens emphasis.**

Keep existing provider and rate-card functionality reachable and tested, but
move its primary narrative to Run Control and Change Economics. Do not delete
the underlying commands or storage in this task.

- [ ] **Step 5: Run navigation and route tests.**

```bash
pnpm --dir apps/desktop exec playwright test e2e/route-loading.spec.ts e2e/runs-design-system.spec.ts e2e/ui-audit.spec.ts
```

Expected: all primary routes load, legacy aliases still resolve, and no
deprecated route is rendered as a competing primary destination.

- [ ] **Step 6: Commit navigation convergence.**

```bash
git add apps/desktop/src/app apps/desktop/src/features/history \
  apps/desktop/src/features/outcomes apps/desktop/src/features/audit \
  apps/desktop/src/features/models-cost
git commit -m "refactor: center navigation on engineering decisions"
```

### Task 6: Add browser verification for the new product promise

**Files:**
- Create: `apps/desktop/e2e/adaptive-verification.spec.ts`
- Create: `apps/desktop/e2e/run-control.spec.ts`
- Modify: `apps/desktop/e2e/work-golden-path.spec.ts`
- Modify: `apps/desktop/e2e/work-design-system.spec.ts`
- Modify: `apps/desktop/e2e/ui-audit.spec.ts`

- [ ] **Step 1: Add the failing scenario tests.**

Cover these user-visible scenarios:

```text
1. Open an active Work Item and see “What should happen next?” with spend and recommendation.
2. Accept a targeted verification recommendation and see a Decision Receipt.
3. Defer an expensive check and see visible verification debt.
4. Open the receipt and verify tree identity, policy, evidence, and unknown values.
5. See a blocked/stale state without a false green completion label.
6. See a no-project/no-task state with one clear next action.
7. Navigate to Runs and find the same evidence without duplicating a dashboard.
```

- [ ] **Step 2: Run the new tests and confirm failure.**

```bash
pnpm --dir apps/desktop exec playwright test e2e/adaptive-verification.spec.ts e2e/run-control.spec.ts
```

Expected: failures identify the missing UI and mocked IPC contracts.

- [ ] **Step 3: Add deterministic browser fixtures/mocks.**

Use the existing E2E harness conventions. Mock only the new verification
commands and keep the existing workflow fixtures authoritative for project,
task, Git, and run state.

- [ ] **Step 4: Make the scenarios pass without weakening assertions.**

Assert accessible names, visible reasons, exact unknown/debt copy, and receipt
fields. Do not assert only CSS classes or screenshot pixels.

- [ ] **Step 5: Run the full desktop gate.**

```bash
pnpm --dir apps/desktop build
pnpm --dir apps/desktop exec playwright test
```

Expected: TypeScript/build/performance checks and all browser tests pass.

- [ ] **Step 6: Commit the product acceptance tests.**

```bash
git add apps/desktop/e2e
git commit -m "test: verify adaptive work control flow"
```

### Task 7: Documentation, release notes, and final verification

**Files:**
- Modify: `README.md`
- Modify: `docs/ENGINEERING_INTELLIGENCE.md`
- Modify: `docs/UI_UX_PRODUCT_PLAN.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/NEXT_DEVELOPMENT_PLAN.md` only where the new Phase 0/1 slice changes sequencing.

- [ ] **Step 1: Document the user-facing product promise.**

Update the README to describe RepoDesk as the AI Engineering Control Plane
around trusted software changes and link to the approved design spec. Keep the
RepoPilot boundary explicit.

- [ ] **Step 2: Document the data/evidence contract.**

Update Engineering Intelligence with Decision Receipts, accepted-change
economics, unknown values, and recommendation-first policy behavior. Update
the UI/UX plan with the Work control hierarchy and accessibility rules.

- [ ] **Step 3: Run repository-wide verification.**

```bash
./scripts/verify-all.sh
```

If the script depends on unavailable native tooling, run the documented
fallback commands individually and record the exact limitation; do not claim
the full gate passed without the command output.

- [ ] **Step 4: Inspect the final diff and evidence.**

```bash
git diff origin/main...HEAD --stat
git diff origin/main...HEAD --check
git status --short --branch
```

Confirm unrelated user changes are absent, no secrets or raw prompt bodies are
introduced, and every new UI action has a backend evidence path.

- [ ] **Step 5: Commit documentation and final verification record.**

```bash
git add README.md docs/ENGINEERING_INTELLIGENCE.md \
  docs/UI_UX_PRODUCT_PLAN.md CHANGELOG.md docs/NEXT_DEVELOPMENT_PLAN.md
git commit -m "docs: describe AI work control plane"
```

## Plan self-review

### Spec coverage

- Product category and target user: Executive decision, market validation, and
  product boundary.
- RepoPilot/RepoDesk/agent boundary: Product boundary section and Task 3.
- Adaptive Verification inputs/outputs/policies: Task 2 and Task 3.
- Decision Receipt: Task 1 and Task 3.
- Event substrate and bounded telemetry: Task 3.
- Work Item control UI/UX: Task 4.
- Navigation convergence: Task 5.
- Product acceptance scenarios: Task 6.
- Phase 0/1 success criteria: Tasks 1–7.
- Phase 2/3: explicitly outside this plan and reserved for follow-up plans.

### Scope and consistency checks

- The plan does not create a second event ledger or analytics database.
- The plan does not make UI claims about pause/stop capabilities that the
  backend does not expose; controls are conditional on existing commands.
- The plan keeps recommendation-first behavior consistent across Rust policy,
  Tauri commands, and React UI.
- Cost and test counts remain optional where source evidence is unavailable.
- `DecisionReceipt` is the canonical RepoDesk decision artifact; RepoPilot's
  `ChangeProof` remains the code-analysis artifact.
- The UI redesign is limited to the Work control slice and navigation
  convergence; it does not rewrite the Code editor or all legacy CSS in one
  untestable change.

## Execution handoff

Implement this plan with fresh review checkpoints after Tasks 3, 4, and 6.
Do not start Phase 2/3 work until the Phase 1 acceptance scenarios pass and
the product has been exercised with real agent runs.
