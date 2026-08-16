# Execution Attribution Truth — Implementation Plan

Date: 2026-08-16
Design: `docs/superpowers/specs/2026-08-16-execution-attribution-truth-design.md`
Branch: `feat/execution-attribution-truth`

## Goal

Make producer attribution a typed, conservative evidence dimension carried from execution through review/commit policy. Supported RepoDesk-managed isolated coding-agent worktrees may prove `exact_isolated`; weaker/manual/legacy paths remain explicit. Existing Projects stay backward compatible unless they enable `require_exact_attribution_for_commit`.

## Invariants

- A recorded run is not automatically exact attribution.
- `exact_clean_workspace` exists only as a reserved semantic state; this cut must not emit it.
- Historical evidence missing attribution defaults to `legacy_unknown`.
- Complete ChangeSet evidence and producer attribution remain separate dimensions.
- Project policy, not execution success, decides whether exact attribution is required for commit.
- Changes and Finish consume the same Safe Commit Manifest; no second commit gate is introduced.
- Attribution classification lives in Rust core; React must render typed state, not infer it from free-form strings.
- No new database, ledger, receipt file, or policy store.

---

## Task 1 — RED: establish the canonical attribution type and classifier contract

### Files

- Create: `crates/repodesk-core/src/change_attribution.rs`
- Modify: `crates/repodesk-core/src/lib.rs`
- Test: module tests in `crates/repodesk-core/src/change_attribution.rs`

### Tests first

Add failing tests for:

1. matching managed worktree + complete ChangeSet evidence -> `ExactIsolated`;
2. worktree `run_id` mismatch -> never exact;
3. worktree `step_id` mismatch -> never exact;
4. missing/blank baseline -> never exact;
5. complete non-isolated execution evidence -> `DerivedPrePost`;
6. incomplete/unavailable ChangeSet evidence -> cannot become exact or derived;
7. manual execution -> `Manual`;
8. default attribution evidence -> `LegacyUnknown`;
9. `ExactCleanWorkspace` is recognized as exact by policy semantics but has no classifier branch that emits it;
10. aggregation of multiple producing steps is conservative: all-compatible exact isolated evidence may remain exact; mixed/incompatible/manual/unknown evidence downgrades.

### Implementation

Introduce:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChangeAttributionStrength {
    ExactIsolated,
    ExactCleanWorkspace,
    DerivedPrePost,
    Manual,
    Unattributed,
    #[default]
    LegacyUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ChangeAttributionEvidence {
    #[serde(default)]
    pub strength: ChangeAttributionStrength,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub baseline_commit: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}
```

Keep classification functions pure. Inputs must be explicit: run/step identity, execution mode, `ChangeEvidenceStatus`, and optional `RunWorktree`. Bound/sanitize downgrade reasons before persistence. Add `is_exact()` and conservative aggregation helpers.

### Verification

Run focused Rust tests for the new module and `cargo fmt --all -- --check`.

---

## Task 2 — RED/GREEN: carry attribution through executor and orchestration evidence

### Files

- Modify: `crates/repodesk-core/src/executors.rs` and/or focused executor finalization modules used on current `main`
- Modify: `crates/repodesk-core/src/executors/runtime.rs`
- Modify: `crates/repodesk-core/src/orchestrator/types.rs`
- Modify: `crates/repodesk-core/src/orchestrator/runner.rs`
- Modify: `crates/repodesk-core/src/orchestrator/execution_evidence.rs`
- Tests: existing executor/orchestrator tests plus focused attribution regression tests

### Tests first

Add failing assertions that:

- an isolated coding-agent result carries `ExactIsolated` with matching `workspace_id` and `base_commit`;
- a non-isolated recorded write path with complete pre/post evidence carries `DerivedPrePost` only when complete evidence actually exists;
- post-launch provenance failure cannot leave an exact claim behind;
- analysis-only/non-write steps do not need exact attribution;
- attribution reasons remain bounded and do not serialize absolute worktree paths as the normal explanation.

### Implementation

Add `change_attribution: ChangeAttributionEvidence` to `SubAgentResult` with `#[serde(default)]`.

Classify attribution at the evidence boundary after ChangeSet capture, using the actual `RunWorktree` metadata already attached to coding-agent execution. Never reconstruct exact attribution later from `run_id` alone.

Where execution issues invalidate provenance, downgrade attribution before building the durable result.

### Verification

Run focused executor/orchestrator tests and Clippy for `repodesk-core`.

---

## Task 3 — RED/GREEN: persist backward-compatible step attribution in TaskRunReceipt

### Files

- Modify: `crates/repodesk-core/src/workflow/receipt.rs`
- Modify: `crates/repodesk-core/src/orchestrator/execution_evidence.rs`
- Test: `crates/repodesk-core/tests/execution_evidence_truth.rs`
- Add focused compatibility tests if existing receipt test module is insufficient

### Tests first

Prove:

- old receipt JSON without attribution deserializes to `LegacyUnknown`;
- new receipt JSON round-trips attribution evidence;
- receipt construction copies attribution from the exact `SubAgentResult`;
- a run ID with no attribution field remains legacy/unknown rather than being upgraded;
- `ExecutionReceipt::succeeded()` semantics are unchanged: complete write ChangeSet evidence is required, but exact attribution is not globally required.

### Implementation

Add to `StepReceipt`:

```rust
#[serde(default)]
pub change_attribution: ChangeAttributionEvidence,
```

Update receipt matching/recovery equality logic so attribution is part of evidence identity. Preserve the existing `Incomplete` versus `RecoveryRequired` distinction.

### Verification

Run workflow/evidence tests and serialization compatibility tests.

---

## Task 4 — RED/GREEN: make ChangeSet Passport consume canonical attribution

### Files

- Modify: `crates/repodesk-core/src/engineering/changeset_passport.rs`
- Modify exports in `crates/repodesk-core/src/engineering.rs` or current engineering module root
- Tests: passport module tests

### Tests first

Replace old `RecordedRun | Manual | Unattributed` expectations with:

- exact isolated receipt -> exact passport attribution;
- derived receipt -> derived passport attribution;
- manual receipt -> manual;
- old receipt -> legacy unknown;
- absence of receipt/current producer evidence -> unattributed;
- multiple write steps aggregate conservatively;
- no test may derive exact from `governance.origin.execution_id` alone.

### Implementation

Delete the local coarse `ChangeAttributionStrength` enum from `changeset_passport.rs`. Reuse the shared core attribution type and evidence. The passport may add concise deterministic explanation fields, but must remain a derived read model.

Prefer receipt attribution over stringly `execution_mode` heuristics. Governance origin remains useful identity metadata, not attribution proof.

### Verification

Run passport/engineering tests.

---

## Task 5 — RED/GREEN: add backward-compatible Project trust policy

### Files

- Modify: `crates/repodesk-core/src/projects.rs`
- Modify the Tauri project command/API layer that serializes `ProjectConfig`
- Modify: `apps/desktop/src/shared/api/api.ts` and/or current shared IPC type owner
- Tests: project config serialization/deserialization and command tests

### Tests first

Prove:

- legacy `project.toml` without trust policy loads with `require_exact_attribution_for_commit = false`;
- a saved true policy round-trips;
- malformed policy fails project config load explicitly;
- project setup defaults to permissive attribution policy;
- policy is project scoped and does not appear in global credential/provider settings.

### Implementation

Add:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectTrustPolicy {
    #[serde(default)]
    pub require_exact_attribution_for_commit: bool,
}
```

and `#[serde(default)] pub trust_policy: ProjectTrustPolicy` to `ProjectConfig`.

Add the narrowest existing project-update command/mutation path possible. Do not create a second project config store or a generic untyped settings blob.

### Verification

Run project config/command tests and frontend typecheck/build after IPC type changes.

---

## Task 6 — RED/GREEN: bind attribution and policy into Safe Commit Manifest

### Files

- Modify: `crates/repodesk-core/src/engineering/safe_commit_manifest.rs`
- Modify manifest construction call sites if needed
- Tests: Safe Commit Manifest module tests

### Tests first

Add failing tests for:

1. permissive policy + derived attribution -> warning, not blocker;
2. exact-required policy + `ExactIsolated` -> attribution gate passes;
3. exact-required policy + `ExactCleanWorkspace` -> policy semantics accept it, while no runtime classifier emits it;
4. exact-required policy + derived/manual/unattributed/legacy -> blocker;
5. already committed historical state + current exact-required mismatch -> warning only;
6. manifest digest changes if attribution evidence changes;
7. manifest digest changes if attribution policy changes;
8. ChangeSet/receipt without attribution cannot silently become commit-ready under exact-required policy.

### Implementation

Bump `SAFE_COMMIT_MANIFEST_VERSION`.

Add typed attribution and policy requirement to `SafeCommitManifest` and `ManifestDigestPayload`. Load the active Project policy in the canonical manifest loader; do not pass policy independently in Changes and Finish.

Use one deterministic blocker string for exact-policy failure. Use warnings for weak attribution when policy is permissive.

### Verification

Run Safe Commit Manifest, Finish, and Changes governance tests to prove `Changes readiness == Finish readiness` remains intact.

---

## Task 7 — RED/GREEN: expose exact-attribution policy in Projects

### Files

- Modify the existing Projects feature under `apps/desktop/src/features/projects/`
- Modify the project-domain hook/API owner used by Projects
- Modify shared styles only where an existing semantic class can be reused; do not add another `*-vN` stylesheet
- Tests: `apps/desktop/e2e/` project/settings ownership tests
- Native E2E project fixture/test if practical with current harness

### Tests first

Playwright must fail until:

- Projects exposes `Require exact producer attribution before commit`;
- persisted true/false values hydrate correctly;
- changing the control calls only the project-scoped mutation path;
- Settings contains no copy/control for this policy;
- mutation failure is visible and does not optimistically pretend the policy changed.

### Implementation

Place the control beside other repository engineering policy, not credentials/global app preferences. Copy should explain that RepoDesk-managed isolated agent execution can satisfy exact attribution while manual/derived changes cannot.

Serialize project mutations to avoid competing writes, following existing project activation mutation discipline.

### Verification

Run focused Playwright and frontend production build. Add one native persistence read/write assertion if it is stable without launching a real external agent.

---

## Task 8 — RED/GREEN: render attribution truth in Changes

### Files

- Modify the existing Changes feature under `apps/desktop/src/features/changes/`
- Modify current ChangeSet Passport / Safe Commit Manifest frontend types in shared API/IPC
- Reuse `apps/desktop/src/app/styles/changes-evidence.css` or current semantic owner; do not create another visual generation
- Tests: `apps/desktop/e2e/` Changes/Work golden-path specs and fixtures

### Tests first

Cover:

- `ExactIsolated` renders `Exact · isolated worktree` and bounded run/baseline identity;
- `DerivedPrePost` renders `Derived · pre/post Git evidence` and is not labelled exact;
- `Manual` renders `Manual handoff`;
- `LegacyUnknown` renders `Unknown · legacy evidence`;
- exact-required weak attribution yields one actionable commit blocker with rerun guidance;
- permissive weak attribution remains visible as a warning rather than a commit blocker;
- normal UI does not render absolute worktree paths.

### Implementation

Render typed state with an exhaustive mapping local to presentation labels only. Do not use `statusTone()` substring heuristics to decide attribution semantics. The gate state comes from `SafeCommitManifest`; React must not recompute whether commit is allowed.

### Verification

Run focused Playwright + frontend build.

---

## Task 9 — Architecture non-regression ratchet

### Files

- Modify: `scripts/check-source-architecture.mjs`
- Modify/add its existing unit tests

### RED tests

Add contract checks so future code cannot:

- reintroduce `RecordedRun` as a ChangeSet attribution state;
- place `require_exact_attribution_for_commit` under Settings ownership;
- introduce frontend string-substring attribution classification;
- make ChangeSet Passport define a duplicate attribution enum instead of importing the canonical core type;
- make Safe Commit Manifest omit attribution policy/evidence from its digest contract.

Keep the existing 28 KiB source-size ratchet unchanged.

### Verification

Run the architecture checker tests and the checker itself against the branch.

---

## Task 10 — Full self-review and exact-head verification

### Self-review checklist

- Search all new attribution constructors for accidental evidence upgrades.
- Confirm no normal UI absolute worktree path leaks.
- Confirm bounded downgrade reasons.
- Confirm old receipts/project configs deserialize conservatively.
- Confirm `exact_clean_workspace` is never emitted by runtime code.
- Confirm mixed multi-writer evidence cannot remain exact unless compatible.
- Confirm project policy has exactly one mutation/ownership surface in Projects.
- Confirm Changes and Finish use one manifest gate.
- Confirm no new versioned CSS, inline styling debt, or duplicate state classifier was added.
- Check all touched files against the source-size ratchet.

### Required exact-head gates

Run/require the repository's full GitHub verification suite on the final head:

- Architecture Ratchet
- frontend production build/performance budgets
- `cargo fmt --all -- --check`
- Clippy with warnings denied
- Rust tests
- coverage
- Playwright mock-IPC E2E
- native Tauri/WebDriverIO E2E
- cargo-deny
- gitleaks / secret gates

If any gate fails, inspect the exact failed job logs, identify root cause, fix, and rerun on the new exact head. Do not merge a head whose checks belong to an earlier commit.

### Delivery

Open/update one PR from `feat/execution-attribution-truth` to `main`. Keep the PR scope to attribution truth/policy/UI evidence; do not fold Cut F design-system cleanup into this PR. Squash-merge only after the exact final head is green and mergeable.

---

## Definition of done

- `recorded_run` no longer exists as a misleading attribution claim.
- isolated supported agent runs can prove `exact_isolated` from first-class evidence.
- weak/manual/legacy attribution is explicit and conservative.
- old data remains readable but never silently strengthened.
- Projects can require exact producer attribution before commit.
- one Safe Commit Manifest applies the policy for both Changes and Finish.
- Changes explains producer attribution and its gate consequence without free-form-log inference.
- architecture tests prevent ownership/classifier regressions.
- exact final head passes all repository gates before squash merge.
