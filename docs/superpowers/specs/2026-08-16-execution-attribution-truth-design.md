# Execution Attribution Truth — Design

Date: 2026-08-16

## Context

RepoDesk already has the evidence needed to make producer attribution truthful:

- supported write-capable coding agents run in RepoDesk-managed detached Git worktrees;
- `RunWorktree` records `workspace_id`, `run_id`, `step_id`, `path`, `base_commit`, creation time, and recovery metadata;
- executor results carry changed paths, changeset evidence status, diff receipt path, execution issues, and optional worktree identity;
- workflow receipts require complete ChangeSet provenance before a write-capable step can count as successful;
- Changes and Finish already consume canonical receipt/tree/scope/verification/acceptance evidence through the ChangeSet Passport and Safe Commit Manifest.

The remaining gap is semantic. `ChangeSetPassport` currently collapses producer evidence to `recorded_run | manual | unattributed`. A recorded run proves execution identity, but it does not prove that every changed byte was exclusively produced by that executor.

RepoDesk must not overstate producer attribution.

## Product decision

Use policy-driven enforcement.

Attribution strength is always rendered truthfully. Weak attribution blocks commit only when the active Project explicitly requires exact attribution. Existing projects remain backward compatible.

This avoids two bad extremes: globally requiring exact attribution would break legitimate manual/legacy workflows, while informational-only attribution would prevent Projects from enforcing trustworthy-change policy.

## Goals

1. Make attribution a typed first-class evidence dimension.
2. Distinguish exact isolated execution from weaker pre/post inference and manual handoff.
3. Never upgrade evidence strength from metadata that cannot prove exclusivity.
4. Carry attribution through run evidence, workflow receipt, ChangeSet Passport, Changes, and Safe Commit Manifest.
5. Allow Projects to require exact attribution for commit readiness.
6. Preserve backward compatibility for historical receipts and existing project configs.
7. Add tests/ratchets that prevent Boolean or stringly-typed attribution shortcuts.

## Non-goals

- no new event ledger, database, or receipt file;
- no autonomous push/merge behavior;
- no OS sandbox claim;
- no authorship inference from model output text;
- no broad UI redesign in this cut;
- no claim that a clean ordinary checkout is exact without proven exclusive workspace ownership.

## Canonical attribution model

Introduce one shared core enum:

```text
exact_isolated
exact_clean_workspace
derived_pre_post
manual
unattributed
legacy_unknown
```

### `exact_isolated`

May be emitted only when all of these are true:

- the write-capable executor ran in a RepoDesk-managed `RunWorktree`;
- worktree metadata matches the current `run_id` and `step_id`;
- a baseline commit was recorded before launch;
- step ChangeSet capture is `complete`;
- produced paths are derived from that isolated worktree relative to the recorded baseline;
- no evidence-integrity issue invalidates the workspace/provenance chain.

This is the only exact state the first implementation is expected to produce.

### `exact_clean_workspace`

Reserved for a future mode where RepoDesk can prove both a clean baseline and exclusive ownership/lease of the execution workspace for the entire run.

A clean normal checkout alone is insufficient because concurrent human/tool writes cannot be excluded. The enum may exist now, but classification must not emit it until exclusivity evidence exists.

### `derived_pre_post`

RepoDesk has complete before/after Git evidence and a recorded executor run, but workspace exclusivity cannot be proven. The delta is useful evidence, not exact authorship.

### `manual`

The change entered through an explicit Manual Handoff/import path. RepoDesk can bind review and verification to the resulting exact tree without claiming an exact external producer.

### `unattributed`

A current ChangeSet exists but producer/run evidence is insufficient.

### `legacy_unknown`

Historical serialized evidence predates attribution. Deserialization is conservative: old evidence never becomes exact automatically.

## Evidence representation

Attribution is captured at the step/run boundary, not reconstructed in UI code.

A focused core structure should carry the proof used to classify a producing step:

```rust
pub struct ChangeAttributionEvidence {
    pub strength: ChangeAttributionStrength,
    pub workspace_id: Option<String>,
    pub baseline_commit: Option<String>,
    pub reason: Option<String>,
}
```

Rules:

- `workspace_id` and `baseline_commit` are evidence metadata;
- `reason` is bounded and sanitized, and explains downgrades only;
- historical absence defaults to `legacy_unknown`;
- non-write steps do not need exact producer attribution;
- UI and IPC consumers use the shared type and never classify arbitrary strings themselves.

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
+ matching run/step identity
=> exact_isolated
```

For a recorded non-isolated run:

```text
recorded executor
+ complete pre/post ChangeSet evidence
+ no proven exclusive workspace
=> derived_pre_post
```

Unavailable/legacy ChangeSet evidence can never be upgraded merely because a run ID exists.

## Receipt compatibility

Extend `StepReceipt` with attribution evidence using `#[serde(default)]` so historical receipts deserialize as `legacy_unknown`.

`ExecutionReceipt::succeeded()` keeps its current complete-ChangeSet requirement. Exact attribution is not globally required for execution success.

The separation is deliberate:

- execution success: did the executor complete with trustworthy ChangeSet capture?
- attribution: how strongly can RepoDesk connect the ChangeSet to producer(s)?
- project policy: is that strength sufficient to commit?

## ChangeSet-level aggregation

The aggregate is deterministic and conservative.

For write-capable steps that actually contribute changed paths:

1. no contributing write step -> attribution is not required for the run;
2. all contributing steps `exact_isolated` -> aggregate `exact_isolated`;
3. all contributing steps `exact_clean_workspace` -> aggregate `exact_clean_workspace`;
4. a mix of exact states, or any `derived_pre_post`, with complete global pre/post evidence -> aggregate `derived_pre_post`;
5. all contributing steps `manual` -> aggregate `manual`;
6. manual mixed with agent-produced changes -> `derived_pre_post` only when complete global pre/post evidence exists, otherwise `unattributed`;
7. any current contributor with insufficient producer evidence -> `unattributed`;
8. if the only missing classification is historical absence -> `legacy_unknown`.

`legacy_unknown` is a compatibility state, not a numeric rank. Attribution is never averaged or converted into an “AI confidence” score.

## ChangeSet Passport

Replace the coarse `RecordedRun | Manual | Unattributed` projection with the shared attribution evidence.

Expose:

- attribution strength;
- producing run ID;
- baseline commit when known;
- isolated workspace identity when relevant;
- a concise mechanically-derived explanation for the Why inspector.

The passport stays a derived read model, never a new persistence authority.

## Project policy

Extend `ProjectConfig` with backward-compatible trust policy owned by Projects:

```rust
#[derive(Default, Serialize, Deserialize)]
pub struct ProjectTrustPolicy {
    #[serde(default)]
    pub require_exact_attribution_for_commit: bool,
}

#[serde(default)]
pub trust_policy: ProjectTrustPolicy,
```

Default is `false`.

Projects exposes an explicit repository policy:

> Require exact producer attribution before commit

The UI explains that supported isolated coding-agent runs satisfy it; manual and inferred pre/post changes do not.

Settings must not own this policy.

## Safe Commit Manifest

Add attribution and attribution policy to the deterministic manifest payload and bump the manifest version.

Pre-commit rules:

- policy `false`: weak attribution is visible as a warning but is not an independent blocker;
- policy `true`: only an exact state satisfies the attribution gate;
- first implementation emits `exact_isolated` as the supported exact state;
- `exact_clean_workspace` is accepted by policy semantics but cannot yet be produced;
- `derived_pre_post`, `manual`, `unattributed`, and `legacy_unknown` block when exact attribution is required;
- already-committed historical state is not rewritten by a later policy change; mismatch is a warning, matching existing scope/acceptance historical semantics.

`Changes readiness == Finish readiness` remains invariant because both consume the same Safe Commit Manifest.

## UI / UX

This cut changes evidence semantics, not the entire visual system.

Changes renders one compact attribution row:

```text
Producer attribution   Exact · isolated worktree
                       Run abc123 · baseline 7f8…
```

Other states:

```text
Producer attribution   Derived · pre/post Git evidence
Producer attribution   Manual handoff
Producer attribution   Unknown · legacy evidence
```

When policy blocks commit:

```text
Commit blocked: this Project requires exact producer attribution.
Rerun the change with a supported isolated coding agent.
```

`derived_pre_post` must never receive exact/green copy merely because it is valid evidence.

Runs may expose immutable attribution evidence for debugging. Mutable policy/review/commit ownership stays in Projects/Changes.

## Why inspector

Explanation is deterministic:

- exact -> managed worktree identity + baseline + complete ChangeSet capture;
- derived -> complete pre/post evidence, exclusivity not proven;
- manual -> explicit handoff/import;
- unknown -> producer/evidence metadata absent or historical.

No LLM-generated explanation is required.

## Error handling

Fail closed on evidence corruption, not on policy weakness.

- mismatched worktree `run_id` / `step_id` -> never exact; persist bounded evidence issue;
- missing baseline on a managed-worktree claim -> never exact;
- incomplete ChangeSet capture -> existing execution-evidence rules block review and attribution cannot be exact;
- old receipt with no attribution -> `legacy_unknown`;
- malformed project trust policy -> project config load fails explicitly;
- never fabricate `derived_pre_post` when underlying before/after evidence is unavailable.

## Security/privacy

- never persist credentials or secret material in attribution reasons;
- do not expose arbitrary absolute worktree paths in normal Changes UI;
- workspace IDs and commit SHAs are valid evidence identifiers;
- reuse existing bounded/sanitized diagnostic conventions;
- exact producer attribution does not imply OS/process isolation.

## Testing

### Core

1. matching managed worktree + complete evidence -> `exact_isolated`;
2. mismatched run/step -> never exact;
3. missing baseline -> never exact;
4. complete non-isolated pre/post -> `derived_pre_post`;
5. manual handoff -> `manual`;
6. historical receipt -> `legacy_unknown`;
7. multi-writer aggregation follows the deterministic rules above;
8. non-write steps do not require exact attribution.

### Compatibility

- old receipt JSON without attribution deserializes;
- new attribution round-trips;
- exact claim cannot be reconstructed from run ID alone;
- old project config without `trust_policy` defaults to permissive behavior.

### Safe Commit Manifest

- default policy + derived attribution -> warning, not blocker;
- exact-required + `exact_isolated` -> attribution gate passes;
- exact-required + derived/manual/unknown/legacy -> blocked;
- committed historical state surfaces later policy mismatch as warning;
- manifest digest changes when attribution or policy changes.

### UI

Playwright covers:

- exact isolated label;
- derived state is not represented as exact;
- exact-required policy gives one actionable commit blocker;
- legacy evidence displays unknown;
- Projects owns the policy control and Settings does not.

Native E2E should cover one persisted project-policy toggle/read path if current fixtures support it without brittle external-agent dependencies.

### Architecture ratchet

Prevent regressions where:

- Settings owns `require_exact_attribution_for_commit`;
- frontend code recreates attribution classification via string matching;
- Passport/Manifest diverge from the shared core type;
- the old `RecordedRun` attribution state returns.

## Implementation boundaries

Expected primary areas:

- a small focused core attribution module;
- `orchestrator/types.rs` and coding-agent result finalization;
- `workflow/receipt.rs`;
- `engineering/changeset_passport.rs`;
- `engineering/safe_commit_manifest.rs`;
- `projects.rs`;
- Projects/Changes IPC + UI;
- Playwright/native fixtures/tests;
- `scripts/check-source-architecture.mjs`.

Do not introduce another persisted ChangeSet or policy store.

## Delivery sequence

1. RED tests for attribution classification and legacy deserialization.
2. Typed attribution through executor -> receipt.
3. RED/green ChangeSet Passport aggregation.
4. Backward-compatible Project trust policy.
5. RED/green Safe Commit Manifest binding + version bump.
6. Changes/Projects UI.
7. Architecture ratchet.
8. Focused Rust/frontend verification.
9. Exact-head full CI, Architecture Ratchet, security, Playwright, and native E2E before merge.

## Acceptance contract

Complete when:

1. “recorded run” is never treated as exact attribution;
2. supported isolated coding-agent execution produces evidence-backed `exact_isolated`;
3. weaker/manual/legacy paths remain explicit;
4. historical receipts/configs stay readable without stronger claims;
5. Projects can require exact attribution for commit;
6. Changes and Finish enforce the same policy through one Safe Commit Manifest;
7. UI can explain attribution strength without parsing free-form logs;
8. no second attribution classifier or persistence authority exists.
