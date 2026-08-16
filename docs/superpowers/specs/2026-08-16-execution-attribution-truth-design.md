# Execution Attribution Truth — Design

Date: 2026-08-16

## Context

RepoDesk already captures strong execution evidence:

- write-capable coding agents run in RepoDesk-managed detached Git worktrees when supported;
- `RunWorktree` records `workspace_id`, `run_id`, `step_id`, `path`, `base_commit`, creation time, and recovery metadata;
- executor results carry changed paths, changeset evidence status, diff receipt path, execution issues, and optional worktree identity;
- the workflow receipt requires complete changeset provenance before a write-capable step can count as successful;
- Changes and Finish already consume canonical receipt/tree/scope/verification/acceptance evidence through the ChangeSet Passport and Safe Commit Manifest.

The remaining gap is semantic: `ChangeSetPassport` currently collapses attribution to `recorded_run | manual | unattributed`. A recorded run proves that RepoDesk has an execution identity, but it does not by itself prove that every changed byte was produced exclusively by that executor.

RepoDesk must not overstate producer attribution.

## Product decision

Use policy-driven enforcement.

Attribution strength is always rendered truthfully, but weak attribution blocks commit only when the active Project explicitly requires exact attribution. Existing projects remain backward compatible.

This is preferable to either making exact attribution mandatory globally, which would break legitimate manual/legacy workflows, or making attribution informational only, which would leave Projects unable to enforce trustworthy-change policy.

## Goals

1. Make attribution a typed first-class evidence dimension.
2. Distinguish exact isolated execution from weaker pre/post inference and manual handoff.
3. Never upgrade evidence strength from metadata that cannot prove exclusivity.
4. Carry attribution through run evidence, workflow receipt, ChangeSet Passport, Changes UI, and Safe Commit Manifest.
5. Allow Projects to require exact attribution for commit readiness.
6. Preserve backward compatibility for historical receipts and existing project configs.
7. Add architecture/tests that prevent future Boolean or stringly-typed attribution shortcuts.

## Non-goals

- no new event ledger, database, or receipt file;
- no autonomous push/merge behavior;
- no OS sandbox claim;
- no attempt to prove authorship from model output text;
- no broad UI redesign in this cut;
- no claim that a clean ordinary checkout is exact unless RepoDesk can also prove exclusive workspace ownership.

## Attribution model

Introduce a canonical `ChangeAttributionStrength` shared by core trust projections.

```text
exact_isolated
exact_clean_workspace
 derived_pre_post
manual
unattributed
legacy_unknown
```

Semantics:

### `exact_isolated`

RepoDesk may emit this only when all of the following are true:

- the write-capable executor ran in a RepoDesk-managed `RunWorktree`;
- worktree metadata matches the current run and step;
- a baseline commit is recorded before launch;
- changeset capture for the step is `complete`;
- the produced paths are derived from that isolated worktree relative to its recorded baseline;
- no evidence-integrity issue invalidates the workspace/provenance chain.

This is the primary exact-attribution state for the current product.

### `exact_clean_workspace`

Reserved for a future execution mode where RepoDesk can prove both a clean baseline and exclusive ownership/lease of the execution workspace for the entire run.

A merely clean ordinary working tree is not sufficient because concurrent human/tool writes cannot be excluded. The first implementation must define the enum/state but must not emit it until an exclusivity mechanism exists.

### `derived_pre_post`

RepoDesk has complete before/after Git evidence and a recorded executor run, but the executor ran in a workspace whose exclusivity cannot be proven. The delta is useful evidence but not exact authorship.

### `manual`

The change entered through an explicit Manual Handoff/import path. RepoDesk can bind review/verification to the exact resulting tree, but it does not claim the external executor identity as exact producer evidence.

### `unattributed`

RepoDesk has a current ChangeSet but no sufficient producer/run identity.

### `legacy_unknown`

Historical serialized evidence predates the attribution field. Deserialization must fail conservative: old data never becomes exact automatically.

## Evidence representation

Add attribution evidence at the step/run boundary rather than deriving it only in the UI.

A bounded structure should capture the proof used to classify a write-capable step, for example:

```rust
pub struct ChangeAttributionEvidence {
    pub strength: ChangeAttributionStrength,
    pub workspace_id: Option<String>,
    pub baseline_commit: Option<String>,
    pub reason: Option<String>,
}
```

Rules:

- `workspace_id` and `baseline_commit` are evidence metadata, not UI decoration;
- `reason` is bounded/sanitized and explains downgrades, never contains secret material or unbounded paths/output;
- historical absence defaults to `legacy_unknown`;
- non-write steps may remain `unattributed`/`legacy_unknown` without blocking run success because producer attribution is relevant to produced ChangeSets, not analysis-only execution.

The canonical classification function lives in core and is reused by orchestration receipt construction and trust projections. UI code does not infer attribution from arbitrary strings.

## Data flow

```text
CodingAgentExecution
  -> SubAgentResult
  -> StepReceipt
  -> TaskRunReceipt
  -> ChangeSetPassport
  -> SafeCommitManifest
  -> Changes / Finish
```

For an isolated coding-agent step:

```text
RunWorktree(base_commit, workspace_id, run_id, step_id)
+ complete ChangeSet evidence
+ matching step/run identity
=> exact_isolated
```

For a recorded non-isolated run with complete pre/post capture:

```text
recorded executor run
+ complete ChangeSet evidence
+ no provable exclusive workspace
=> derived_pre_post
```

If evidence capture is unavailable/legacy, attribution must not be upgraded merely because a run ID exists.

## Receipt compatibility

Extend `StepReceipt` with attribution evidence using `#[serde(default)]`.

Historical receipts therefore deserialize as `legacy_unknown`.

`ExecutionReceipt::succeeded()` keeps its existing changeset-completeness requirement. Exact attribution is not globally required for execution success because project policy controls whether weak attribution is acceptable at commit time.

This keeps the separation clear:

- execution success answers whether the executor completed with trustworthy ChangeSet capture;
- attribution answers how strongly RepoDesk can connect that ChangeSet to a producer;
- project policy decides whether that strength is sufficient to commit.

## Run-level attribution projection

Derive one ChangeSet-level strength from write-capable step evidence.

Conservative aggregation:

- no write-capable steps: attribution is not required for the run;
- exactly one producing write step: use that step's strength;
- multiple write steps contributing to the same ChangeSet: the aggregate strength is the weakest contributing attribution state;
- if any contributing step is `legacy_unknown` or `unattributed`, the ChangeSet must not claim exact attribution;
- mixed `manual` + agent-produced changes must be represented as non-exact rather than selecting the stronger agent state.

Do not average attribution or turn it into a numeric score.

## ChangeSet Passport

Replace the current coarse `RecordedRun | Manual | Unattributed` projection with the canonical attribution type/evidence.

The passport should expose:

- attribution strength;
- producing run ID;
- baseline commit when known;
- isolated workspace identity when relevant;
- a concise explanation suitable for the Why inspector.

The passport remains a derived read model, never a second persistence authority.

## Project policy

Extend `ProjectConfig` with a backward-compatible execution/trust policy, defaulting to permissive attribution behavior for existing projects.

Preferred shape:

```rust
#[derive(Default, Serialize, Deserialize)]
pub struct ProjectTrustPolicy {
    #[serde(default)]
    pub require_exact_attribution_for_commit: bool,
}
```

`ProjectConfig` gets:

```rust
#[serde(default)]
pub trust_policy: ProjectTrustPolicy,
```

Default: `false`.

The policy belongs to Projects, not Settings.

Projects UI should expose this as an explicit repository policy with wording such as:

> Require exact producer attribution before commit

The control must explain that supported isolated coding-agent runs satisfy it; manual or inferred pre/post changes do not.

## Safe Commit Manifest

Add attribution to the manifest's deterministic evidence payload and bump its version.

The manifest records:

- current ChangeSet attribution strength/evidence;
- whether project policy requires exact attribution;
- attribution blocker/warning state.

Rules before commit:

- if `require_exact_attribution_for_commit == false`, weak attribution is visible as a warning but does not independently block commit;
- if the policy is `true`, only an exact state may satisfy the gate;
- for the first implementation, `exact_isolated` is the only emitted exact state;
- `exact_clean_workspace` is accepted by policy semantics but cannot currently be produced;
- `derived_pre_post`, `manual`, `unattributed`, and `legacy_unknown` block when exact attribution is required;
- historical already-committed manifests do not retroactively become invalid; current policy mismatch is shown as historical warning, consistent with current scope/acceptance behavior.

`Changes readiness == Finish readiness` remains invariant because both consume the same Safe Commit Manifest.

## UI / UX

This cut changes evidence semantics, not the visual system wholesale.

Changes should render one compact attribution row in the ChangeSet Passport / trust summary:

```text
Producer attribution   Exact · isolated worktree
                       Run abc123 · baseline 7f8…
```

Other examples:

```text
Producer attribution   Derived · pre/post Git evidence
Producer attribution   Manual handoff
Producer attribution   Unknown · legacy evidence
```

When project policy blocks commit:

```text
Commit blocked: this Project requires exact producer attribution.
Rerun the change with a supported isolated coding agent.
```

Avoid green terminology for `derived_pre_post`. It can be valid evidence without being exact evidence.

Runs may expose the same immutable attribution evidence for debugging, but mutable policy/review/commit ownership remains in Changes/Projects.

## Why inspector behavior

The attribution explanation should be mechanically derived from evidence:

- why exact: managed worktree identity + baseline + complete ChangeSet capture;
- why derived: complete pre/post evidence exists but workspace exclusivity is not proven;
- why manual: imported/manual execution mode;
- why unknown: producer/evidence metadata absent or legacy.

No LLM-generated explanation is required.

## Error handling

Fail closed on evidence corruption, not on policy weakness.

Examples:

- mismatched worktree `run_id` / `step_id`: downgrade/block exact classification and persist a bounded evidence issue;
- missing baseline on a claimed managed worktree: cannot emit `exact_isolated`;
- incomplete ChangeSet capture: attribution cannot be exact and existing execution-evidence rules already block review;
- old receipt with no attribution field: `legacy_unknown`;
- malformed project policy: project config load fails explicitly rather than silently changing policy.

Do not fabricate `derived_pre_post` if the underlying before/after evidence was unavailable.

## Security and privacy

- never persist credential/provider secret data in attribution reasons;
- do not expose arbitrary absolute filesystem paths in normal Changes UI;
- workspace IDs and commit SHAs are acceptable evidence identifiers;
- existing bounded/sanitized diagnostic conventions apply;
- attribution does not imply OS/process isolation.

## Testing strategy

### Core unit tests

1. managed matching worktree + complete evidence -> `exact_isolated`;
2. managed worktree with mismatched run/step -> never exact;
3. managed worktree without baseline -> never exact;
4. complete non-isolated pre/post evidence -> `derived_pre_post`;
5. manual handoff -> `manual`;
6. historical receipt -> `legacy_unknown`;
7. multi-writer aggregation returns weakest strength;
8. non-write steps do not make an otherwise valid run attribution-required.

### Receipt compatibility tests

- old JSON without attribution deserializes;
- new attribution round-trips;
- exact claim cannot be reconstructed from only `run_id`.

### Safe Commit Manifest tests

- default project policy + derived attribution -> warning, not blocker;
- exact-required policy + `exact_isolated` -> attribution gate passes;
- exact-required policy + derived/manual/unknown/legacy -> blocked;
- committed historical state surfaces policy mismatch as warning rather than rewriting history;
- manifest digest changes when attribution or attribution policy changes.

### UI tests

Playwright should cover:

- exact isolated attribution label;
- derived attribution is not styled/copy-labeled as exact;
- exact-required policy produces a single actionable commit blocker;
- legacy attribution displays unknown rather than exact/recorded;
- Projects owns the exact-attribution policy control; Settings does not.

Native E2E should cover at least one persisted project-policy toggle/read path if the current native fixture can do so without introducing brittle external-agent dependencies.

### Architecture ratchet

Add non-regression checks so:

- Settings cannot own `require_exact_attribution_for_commit`;
- UI cannot recreate a second attribution classifier from string matching;
- ChangeSet Passport and Safe Commit Manifest use the shared core attribution type;
- the raw `RecordedRun` attribution enum does not return.

## Implementation boundaries

Expected primary files/modules:

- `crates/repodesk-core/src/orchestrator/types.rs`
- executor/finalization code that constructs `SubAgentResult`
- `crates/repodesk-core/src/workflow/receipt.rs`
- a small focused core attribution module rather than growing an existing god-file
- `crates/repodesk-core/src/engineering/changeset_passport.rs`
- `crates/repodesk-core/src/engineering/safe_commit_manifest.rs`
- `crates/repodesk-core/src/projects.rs`
- Projects/Changes IPC types and UI
- Playwright/native fixtures/tests
- `scripts/check-source-architecture.mjs`

Do not introduce another persisted ChangeSet or policy store.

## Delivery sequence

1. RED tests for canonical attribution classification and legacy deserialization.
2. Implement typed attribution evidence through executor -> receipt.
3. RED/green ChangeSet Passport aggregation.
4. Add backward-compatible Project trust policy.
5. RED/green Safe Commit Manifest policy binding and version bump.
6. Surface attribution in Changes and policy in Projects.
7. Add architecture ratchet.
8. Run focused Rust/frontend tests.
9. Run exact-head full CI, Architecture Ratchet, security gates, Playwright, and native E2E before merge.

## Acceptance contract

This cut is complete when:

1. RepoDesk never equates “recorded run” with exact producer attribution.
2. Supported isolated coding-agent execution produces evidence-backed `exact_isolated` attribution.
3. weaker/manual/legacy paths remain explicitly distinguishable.
4. old receipts/configs remain readable without being upgraded to stronger claims.
5. Projects can require exact attribution for commit.
6. Changes and Finish enforce the exact same attribution policy through one Safe Commit Manifest.
7. UI can explain why the attribution has its current strength without parsing free-form logs.
8. no second attribution classifier or persistence authority exists.
