# Measured Verification Catalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox ("- [ ]") syntax for tracking.

**Goal:** Make RepoDesk's verification advisor use stable check descriptors and real per-check execution history while preserving explicit human control over running checks.

**Architecture:** Normalize legacy string checks into a project-scoped descriptor with stable identity, then record bounded per-check facts inside the existing VerificationFinished event. A deterministic history projection replays those events and feeds wall-clock estimates and confidence into the existing adaptive advisor; the desktop only records decisions and never auto-runs checks.

**Tech Stack:** Rust workspace, serde/TOML, chrono, SHA-256, canonical SQLite engineering-event ledger, Tauri IPC, React/TypeScript, TanStack Query, Playwright.

**Spec:** docs/superpowers/specs/2026-09-07-measured-verification-catalog.md

## Global Constraints

- Keep legacy checks = ["..."] project TOML readable and writable.
- Keep old VerificationFinished events readable when check_results is absent.
- Use the canonical append-only event ledger as the only history source.
- Do not infer relevant paths from command text.
- Do not store full stdout/stderr in engineering events.
- Do not invent local verification cost or AI/token spend.
- Recording a recommendation must never execute a check.
- Keep Verify/Finish receipt binding and allowlisted process execution unchanged.
- Treat rust-tauri as a Rust desktop project for new-project defaults.
- Use explicit unknown, provisional, and calibrated confidence states.

---

## File map

| File | Responsibility |
| --- | --- |
| crates/repodesk-core/src/project_checks.rs | Descriptor, compatibility normalization, stable IDs, validation, defaults. |
| crates/repodesk-core/src/projects.rs | Project config integration and catalog mutation helpers. |
| crates/repodesk-core/src/lib.rs | Export the project-check module. |
| crates/repodesk-cli/src/commands/checks.rs | Adapt the existing CLI add/info paths to normalized descriptors. |
| crates/repodesk-core/src/checks.rs | Run normalized descriptors and retain stable IDs/timeouts. |
| crates/repodesk-core/src/engineering/instrumentation.rs | Serialize per-check verification telemetry. |
| crates/repodesk-core/src/workflow/finish.rs | Pass actual command results to telemetry. |
| crates/repodesk-core/src/engineering/verification_history.rs | Replay events into deterministic check history. |
| crates/repodesk-core/src/engineering/mod.rs | Export history types/functions. |
| crates/repodesk-core/src/engineering/adaptive_verification.rs | Consume history and produce honest estimates/confidence. |
| apps/desktop/src-tauri/src/commands/verification.rs | Build the Advisor snapshot from catalog and history. |
| apps/desktop/src-tauri/src/commands/project.rs | Expose explicit project-check setup IPC. |
| apps/desktop/src-tauri/src/lib.rs | Register the new project catalog IPC commands. |
| apps/desktop/src/features/projects/ProjectsTab.tsx | Show catalog state and setup/add-check controls. |
| apps/desktop/src/features/work/VerificationAdvisorCard.tsx | Render duration, confidence, and empty-catalog guidance. |
| apps/desktop/src/shared/api/verification.ts | Add serialized history fields to IPC types. |
| apps/desktop/e2e/fixtures.ts | Add measured/provisional/empty-catalog fixture data. |
| apps/desktop/e2e/adaptive-verification.spec.ts | Verify advisor states and explicit decision behavior. |
| apps/desktop/e2e/projects-design-system.spec.ts | Verify project catalog setup UX. |
| docs/ENGINEERING_INTELLIGENCE.md, docs/UI_UX_PRODUCT_PLAN.md, CHANGELOG.md | Document measured facts and the decision boundary. |

---

### Task 1: Normalize project checks and give them stable identities

**Files:**
- Create: crates/repodesk-core/src/project_checks.rs
- Modify: crates/repodesk-core/src/projects.rs
- Modify: crates/repodesk-core/src/lib.rs
- Modify: crates/repodesk-cli/src/commands/checks.rs
- Modify: apps/desktop/src-tauri/src/commands/project.rs
- Test: crates/repodesk-core/tests/project_checks_contract.rs

**Interfaces:**
- Produces ProjectCheck, normalize_legacy_commands, stable_check_id, default_checks_for_project_type, and explicit catalog mutation helpers.
- ProjectConfig.checks exposes normalized descriptors while TOML accepts legacy strings and structured tables.

- [ ] **Step 1: Write failing compatibility and identity tests**

~~~rust
#[test]
fn legacy_commands_normalize_without_index_based_ids() {
    let first = normalize_legacy_commands(vec!["cargo test --all".into()]).unwrap();
    let reordered = normalize_legacy_commands(vec![
        "cargo fmt --all -- --check".into(),
        "cargo test --all".into(),
    ]).unwrap();

    assert_eq!(first[0].id, reordered[1].id);
    assert_eq!(first[0].timeout_secs, 120);
}

#[test]
fn rust_tauri_has_rust_defaults_for_new_projects() {
    assert_eq!(default_checks_for_project_type("rust-tauri").len(), 3);
}
~~~

- [ ] **Step 2: Run the focused test and confirm it fails**

Run:

~~~bash
cargo test -p repodesk-core --test project_checks_contract
~~~

Expected: compilation or assertion failures because normalized descriptors and the rust-tauri default are not present.

- [ ] **Step 3: Implement the descriptor and compatibility adapter**

Define ProjectCheck with id, title, command, kind, required, relevant_paths, and timeout_secs. Add a custom serde adapter that accepts either a legacy string or a structured TOML table and normalizes both into ProjectCheck values. Validate commands through the existing allowlist, reject blank IDs/commands, reject invalid timeout bounds, normalize repository-relative paths, and derive check-<short-sha256> from the trimmed command when no explicit ID exists.

Use the existing conservative kind classifier for legacy entries and preserve the command as the fallback title. Add rust-tauri to the Rust default match. Reading an unchanged legacy file must not write it back; an explicit catalog edit may serialize structured entries.

Core shape:

~~~rust
pub struct ProjectCheck {
    pub id: String,
    pub title: String,
    pub command: String,
    pub kind: String,
    pub required: bool,
    pub relevant_paths: Vec<String>,
    pub timeout_secs: u64,
}

impl ProjectCheck {
    pub fn new(id: &str, title: &str, command: &str) -> Self;
    pub fn with_required(self, required: bool) -> Self;
    pub fn with_relevant_paths(self, paths: impl IntoIterator<Item = impl Into<String>>) -> Self;
    pub fn with_timeout_secs(self, timeout_secs: u64) -> Self;
}

pub fn normalize_legacy_commands(
    commands: Vec<String>,
) -> RepoDeskResult<Vec<ProjectCheck>>;

pub fn stable_check_id(command: &str) -> String;
~~~

- [ ] **Step 4: Add explicit mutation helpers**

Define the serde adapter's internal untagged RawProjectCheck as either Legacy(String) or Structured(ProjectCheckInput), and make ProjectConfig use that adapter when loading checks. Update add_project_check to accept a descriptor, validate it once, deduplicate by stable ID, and persist the project. Adapt CLI and desktop project-info rendering to display descriptor titles/commands. Add apply_recommended_project_checks, which fills an empty catalog only after an explicit caller request and never runs a command.

- [ ] **Step 5: Run tests and commit**

~~~bash
cargo test -p repodesk-core --test project_checks_contract
cargo test -p repodesk-core projects::tests
git diff --check
git add crates/repodesk-core/src/project_checks.rs crates/repodesk-core/src/projects.rs crates/repodesk-core/src/lib.rs crates/repodesk-cli/src/commands/checks.rs apps/desktop/src-tauri/src/commands/project.rs crates/repodesk-core/tests/project_checks_contract.rs
git commit -m "feat: add stable project check catalog"
~~~

### Task 2: Run descriptors without losing execution facts

**Files:**
- Modify: crates/repodesk-core/src/checks.rs
- Test: crates/repodesk-core/src/checks.rs and crates/repodesk-core/tests/project_checks_contract.rs

**Interfaces:**
- Consumes ProjectCheck from Task 1.
- Produces ChecksRunResult.commands: Vec<CheckCommandResult> with the normalized stable ID, descriptor timeout, timestamps, status, duration, tree identity, and log reference.

- [ ] **Step 1: Write a failing stable-ID execution test**

~~~rust
#[test]
fn run_checks_uses_descriptor_identity() {
    let descriptor = ProjectCheck::new("version", "Version probe", "npm --version")
        .with_timeout_secs(30);
    let result = run_project_check(&descriptor, env::current_dir().unwrap().as_path(), None);
    assert_eq!(result.check_id, "version");
    assert!(result.finished_at >= result.started_at);
}
~~~

- [ ] **Step 2: Run the focused test and confirm the old index ID fails**

~~~bash
cargo test -p repodesk-core run_checks_uses_descriptor_identity
~~~

Expected: FAIL because run_checks currently enumerates strings and emits project-check-{index}.

- [ ] **Step 3: Add the descriptor runner and update run_checks**

Add the private helper run_project_check(check: &ProjectCheck, cwd: &Path, log_ref: Option<&str>) -> CheckCommandResult. It passes check.id and check.timeout_secs into run_validated_check_with_id and attaches log_ref after execution. Iterate normalized descriptors in run_checks, use the helper, and keep summary formatting and aggregate success behavior unchanged.

- [ ] **Step 4: Preserve ad-hoc command callers**

Keep run_validated_check(command, cwd, timeout_secs) for task/orchestrator callers. It may continue deriving an ad-hoc ID from the command; only project-configured checks use descriptor IDs.

- [ ] **Step 5: Run focused execution tests and commit**

~~~bash
cargo test -p repodesk-core checks
cargo test -p repodesk-core --test project_checks_contract
git add crates/repodesk-core/src/checks.rs crates/repodesk-core/tests/project_checks_contract.rs
git commit -m "feat: execute configured checks by stable identity"
~~~

### Task 3: Persist bounded per-check verification telemetry

**Files:**
- Modify: crates/repodesk-core/src/engineering/instrumentation.rs
- Modify: crates/repodesk-core/src/workflow/finish.rs
- Test: crates/repodesk-core/tests/verification_telemetry_contract.rs

**Interfaces:**
- Consumes CheckCommandResult from Task 2.
- Produces an optional check_results JSON array on VerificationFinished without removing success, command_count, evidence refs, or error.

- [ ] **Step 1: Write an event round-trip test**

~~~rust
#[test]
fn verification_finished_contains_bounded_check_results() {
    let event = event_from_telemetry(sample_telemetry());
    let results = event.attributes["check_results"].as_array().unwrap();
    assert_eq!(results[0]["check_id"], "unit-tests");
    assert_eq!(results[1]["status"], "timeout");
    assert!(results[0].get("stdout").is_none());
    assert_eq!(results[0]["tests_observed"], Value::Null);
}
~~~

- [ ] **Step 2: Run the new test and confirm the attribute is absent**

~~~bash
cargo test -p repodesk-core --test verification_telemetry_contract
~~~

Expected: FAIL because VerificationFinishedTelemetry currently contains only aggregate fields.

- [ ] **Step 3: Add bounded telemetry types and serialization**

Add VerificationCheckTelemetry with check_id, command, status, exit_code, duration_ms, started_at, finished_at, tree_identity, log_evidence_ref, and tests_observed. Add from_result(&CheckCommandResult). Extend VerificationFinishedTelemetry with check_results: &[VerificationCheckTelemetry], serializing only the array and never output buffers.

- [ ] **Step 4: Pass actual results from run_verification**

In workflow/finish.rs, construct telemetry from result.commands for successful and aggregate-failure paths. Pass an empty slice for an early run_checks error. Keep telemetry best-effort so a ledger error cannot change the verification result.

- [ ] **Step 5: Run compatibility tests and commit**

~~~bash
cargo test -p repodesk-core --test verification_telemetry_contract
cargo test -p repodesk-core --test core_evidence_workflow
cargo test -p repodesk-core engineering::events
git add crates/repodesk-core/src/engineering/instrumentation.rs crates/repodesk-core/src/workflow/finish.rs crates/repodesk-core/tests/verification_telemetry_contract.rs
git commit -m "feat: record per-check verification facts"
~~~

### Task 4: Build the deterministic per-check history projection

**Files:**
- Create: crates/repodesk-core/src/engineering/verification_history.rs
- Modify: crates/repodesk-core/src/engineering/mod.rs
- Test: crates/repodesk-core/tests/verification_history_contract.rs

**Interfaces:**
- Consumes replayed EngineeringEvent values.
- Produces VerificationCheckHistory, VerificationHistoryConfidence, and derive_verification_history(events).

- [ ] **Step 1: Write projection tests**

Cover no results, one/two/three usable samples, failures, timeout exclusion, latest status, and duplicate (verification_id, check_id) records.

~~~rust
#[test]
fn history_uses_median_and_marks_three_samples_calibrated() {
    let events = events_for_durations("unit-tests", [100, 300, 200]);
    let history = derive_verification_history(&events);
    let check = &history.checks[0];

    assert_eq!(check.measured_runs, 3);
    assert_eq!(check.median_duration_ms, Some(200));
    assert_eq!(check.confidence, VerificationHistoryConfidence::Calibrated);
}
~~~

- [ ] **Step 2: Run the projection tests and confirm they fail**

~~~bash
cargo test -p repodesk-core --test verification_history_contract
~~~

Expected: FAIL because the module and types do not exist.

- [ ] **Step 3: Implement replay and aggregation**

Parse only VerificationFinished events with a valid check_results array. Deduplicate by verification ID plus check ID, retain the newest occurrence, count measured runs and failures, retain latest status/timestamp/tree, and compute the sorted median from passed/failed durations. Keep timeout records in counts/status but exclude them from the estimate. Aggregate-only old events produce unknown per-check history.

~~~rust
pub enum VerificationHistoryConfidence {
    Unknown,
    Provisional,
    Calibrated,
}

pub struct VerificationCheckHistory {
    pub check_id: String,
    pub measured_runs: usize,
    pub failed_runs: usize,
    pub latest_status: Option<String>,
    pub latest_at: Option<DateTime<Utc>>,
    pub latest_tree_identity: Option<String>,
    pub median_duration_ms: Option<u64>,
    pub confidence: VerificationHistoryConfidence,
}
~~~

- [ ] **Step 4: Run projection and advisor contract tests**

~~~bash
cargo test -p repodesk-core --test verification_history_contract
cargo test -p repodesk-core --test verification_telemetry_contract
cargo test -p repodesk-core --test adaptive_verification_contract
~~~

- [ ] **Step 5: Commit the projection**

~~~bash
git add crates/repodesk-core/src/engineering/verification_history.rs crates/repodesk-core/src/engineering/mod.rs crates/repodesk-core/tests/verification_history_contract.rs
git commit -m "feat: derive verification check history"
~~~

### Task 5: Feed measured history into the adaptive advisor

**Files:**
- Modify: crates/repodesk-core/src/engineering/adaptive_verification.rs
- Modify: apps/desktop/src-tauri/src/commands/verification.rs
- Modify: apps/desktop/src/shared/api/verification.ts
- Test: crates/repodesk-core/tests/adaptive_verification_contract.rs
- Test: apps/desktop/src-tauri/src/commands/verification.rs existing unit-test section

**Interfaces:**
- Consumes ProjectCheck and VerificationCheckHistory.
- Extends VerificationCheckCandidate with measured_runs, failed_runs, median_duration_ms, history_confidence, and latest_at while retaining estimated_seconds for legacy callers and fixtures.
- Keeps estimated_cost_units: None.

- [ ] **Step 1: Add advisor tests for measured time/confidence**

~~~rust
#[test]
fn recommendation_uses_measured_median_without_fabricating_cost() {
    let input = input(vec![candidate_with_history(Some(200), 3, "calibrated")]);
    let recommendation = recommend_verification(&input);
    assert_eq!(recommendation.estimated_wall_clock_ms, Some(200));
    assert_eq!(recommendation.estimated_cost_units, None);
    assert_eq!(recommendation.uncertainty_label, "calibrated");
}
~~~

- [ ] **Step 2: Run the advisor contract and confirm it fails**

~~~bash
cargo test -p repodesk-core --test adaptive_verification_contract
~~~

Expected: FAIL until candidate metadata and estimate logic are extended.

- [ ] **Step 3: Extend candidate DTOs and aggregation**

Use median_duration_ms as the precise wall-clock source and retain estimated_seconds as a rounded compatibility field. Return an unknown total if any selected check lacks a usable median; for legacy fixture candidates with only estimated_seconds, preserve the existing seconds-based calculation. Mark confidence unknown/provisional/calibrated from explicit history values instead of treating every non-null estimate as calibrated. Keep required/path/failure decision ordering.

- [ ] **Step 4: Update the Tauri projection**

In work_verification_advisor, normalize project.checks, derive history from already loaded events, and join by stable ID. Set check_history to measured when per-check records exist, partial for aggregate-only legacy events, and unknown when no history exists. Keep RepoPilot and Git source boundaries unchanged.

- [ ] **Step 5: Run tests and commit**

~~~bash
cargo test -p repodesk-core --test adaptive_verification_contract
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml commands::verification
git add crates/repodesk-core/src/engineering/adaptive_verification.rs crates/repodesk-core/tests/adaptive_verification_contract.rs apps/desktop/src-tauri/src/commands/verification.rs apps/desktop/src/shared/api/verification.ts
git commit -m "feat: advise from measured verification history"
~~~

### Task 6: Add explicit catalog setup and empty-state UX

**Files:**
- Modify: apps/desktop/src-tauri/src/commands/project.rs
- Modify: apps/desktop/src-tauri/src/lib.rs
- Modify: apps/desktop/src/features/projects/ProjectsTab.tsx
- Modify: apps/desktop/src/features/work/VerificationAdvisorCard.tsx
- Modify: related project/work CSS only for the new states
- Test: apps/desktop/e2e/projects-design-system.spec.ts
- Test: apps/desktop/e2e/adaptive-verification.spec.ts

**Interfaces:**
- Consumes apply_recommended_project_checks and add_project_check.
- Produces project_apply_recommended_checks and project_add_check; both mutate configuration only and never run commands.

- [ ] **Step 1: Add failing Playwright coverage for an empty catalog**

~~~ts
await expect(page.getByRole("button", { name: "Add recommended checks" })).toBeVisible();
await page.getByRole("button", { name: "Add recommended checks" }).click();
await expect(page.getByText(/checks added/i)).toBeVisible();
await expect(page.getByText(/verification started/i)).toHaveCount(0);
~~~

- [ ] **Step 2: Run the focused tests and confirm the controls are absent**

~~~bash
pnpm --dir apps/desktop e2e e2e/projects-design-system.spec.ts e2e/adaptive-verification.spec.ts
~~~

Expected: FAIL because the IPC actions and controls do not exist.

- [ ] **Step 3: Add explicit project setup commands**

Validate project name and command through the same core allowlist. Add recommended checks only when the catalog is empty and add a single command idempotently. Return updated ProjectConfig for a direct React Query update.

- [ ] **Step 4: Render setup and evidence quality**

Update ProjectConfigSummary.checks to the descriptor shape. In Projects, show title, kind, requiredness, and relevant paths. In Advisor, show duration/sample count/confidence, keep cost explicitly Not measured, and show setup guidance for an empty catalog. Preserve decision recording and evidence inspection.

- [ ] **Step 5: Run frontend checks and commit**

~~~bash
pnpm --dir apps/desktop build
pnpm --dir apps/desktop e2e e2e/projects-design-system.spec.ts e2e/adaptive-verification.spec.ts
git add apps/desktop/src-tauri/src/commands/project.rs apps/desktop/src-tauri/src apps/desktop/src/features/projects/ProjectsTab.tsx apps/desktop/src/features/work/VerificationAdvisorCard.tsx apps/desktop/src/shared/api/verification.ts apps/desktop/e2e/fixtures.ts apps/desktop/e2e/projects-design-system.spec.ts apps/desktop/e2e/adaptive-verification.spec.ts
git commit -m "feat: add verification catalog setup UX"
~~~

### Task 7: Update evidence presentation, docs, and run the full gate

**Files:**
- Modify: apps/desktop/src/features/history/RunsWorkspace.tsx to show per-check facts in the Run evidence view
- Modify: docs/ENGINEERING_INTELLIGENCE.md
- Modify: docs/UI_UX_PRODUCT_PLAN.md
- Modify: CHANGELOG.md

- [ ] **Step 1: Add a Runs evidence panel**

Render stable check ID/title, latest status, measured runs, median duration, confidence, and evidence reference. Keep missing history as Not measured; never convert unknown values to zero.

- [ ] **Step 2: Update documentation**

Document that local verification time comes from actual executions, AI spend is separate, timeouts are excluded from normal estimates, and Advisor recording does not execute checks. Document empty-catalog setup and legacy compatibility.

- [ ] **Step 3: Run focused gates**

~~~bash
cargo test -p repodesk-core --test project_checks_contract
cargo test -p repodesk-core --test verification_telemetry_contract
cargo test -p repodesk-core --test verification_history_contract
cargo test -p repodesk-core --test adaptive_verification_contract
pnpm --dir apps/desktop build
pnpm --dir apps/desktop e2e e2e/projects-design-system.spec.ts e2e/adaptive-verification.spec.ts
~~~

- [ ] **Step 4: Perform the real desktop visual check**

Open Work with a measured fixture and an empty-catalog fixture through the approved desktop-compatible harness. Verify hierarchy between action, time estimate, confidence, local cost unknown, and evidence source. Capture the screenshot for review.

- [ ] **Step 5: Run the repository gate and inspect the final diff**

~~~bash
./scripts/verify-all.sh
git diff origin/main..HEAD --stat
git status --short --branch
~~~

Expected: all checks pass, the worktree is clean, and no unrelated files changed. Push to origin/main only after the final evidence review.

## Acceptance criteria

A configured project can run a check twice, RepoDesk shows its stable identity, latest status, measured duration, and sample count in Work/Runs, and Advisor uses that history for a time estimate without claiming an AI cost. A project with no checks receives a clear setup path rather than a recommendation that pretends verification is available.
