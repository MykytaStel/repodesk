# Language Recovery Vertical Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first working IDE Recovery vertical slice: a centralized backend state model, event-driven language-server diagnosis and verified managed repair, a calm IDE Health surface, and one shared contextual repair flow from the editor.

**Architecture:** `repodesk-core::recovery` owns serialized contracts, deterministic state transitions, attempt history, and the language adapter. A Tauri `RecoveryCoordinator` combines language discovery, live server status, and the existing allowlisted installer, then exposes generic recovery commands and change events. React consumes one recovery snapshot through a provider; the titlebar indicator, IDE Health panel, and editor affordance all open the same record and repair preview.

**Tech Stack:** Rust 2024, serde/chrono, Tauri 2 commands and events, React 18, TypeScript, TanStack Query 5, existing RepoDesk CSS tokens, Playwright.

## Global Constraints

- Normal editing, scrolling, selection, syntax highlighting, and save must remain available while language intelligence is missing, starting, crashed, repairing, or blocked.
- Checks run only on application/project/capability events and explicit `Check again`; there is no continuous background polling.
- Automatic actions are limited to compiled, local, reversible, bounded, non-project-mutating recipes.
- Installs, upgrades, downgrades, downloads, project changes, and shell execution require an explicit preview and confirmation.
- Frontend input contains stable ids and confirmation tokens only; it never supplies an executable, package, URL, or arbitrary arguments.
- Every repair result is verified; a successful installer exit alone cannot produce `healthy`.
- Evidence is redacted and bounded before crossing the Tauri boundary.
- The aggregate indicator is quiet when healthy and counts only `degraded`, `needs_approval`, or `blocked` records.
- Existing editor gutter, line-number navigation, single-scrollbar geometry, library navigation, and language intelligence behavior must not regress.
- This plan does not implement Git, terminal, task-runner, toolchain, or local-AI adapters; each receives a later standalone plan against the same contracts.

---

## File map

### Core recovery domain

- Create `crates/repodesk-core/src/recovery/mod.rs`: public recovery module exports.
- Create `crates/repodesk-core/src/recovery/types.rs`: serialized state, diagnosis, action, preview, attempt, and snapshot contracts.
- Create `crates/repodesk-core/src/recovery/engine.rs`: deterministic record store, generation checks, repair transitions, attempt budget, and bounded history.
- Create `crates/repodesk-core/src/recovery/store.rs`: atomic JSON persistence for latest records and bounded history under the RepoDesk application-data root.
- Create `crates/repodesk-core/src/recovery/language.rs`: pure mapping from language discovery/runtime/install observations to recovery observations.
- Create `crates/repodesk-core/tests/recovery_engine.rs`: public contract and state-machine integration tests.
- Create `crates/repodesk-core/tests/recovery_store.rs`: save/load, corruption, and retention tests.
- Create `crates/repodesk-core/tests/language_recovery.rs`: language adapter mapping and safety tests.
- Modify `crates/repodesk-core/src/lib.rs`: export `recovery`.
- Modify `crates/repodesk-core/src/language_tools.rs`: stream existing status transitions to a supplied observer without changing install policy.
- Modify `crates/repodesk-core/tests/language_tools_security.rs`: prove observer events, verified promotion, and incompatible companion-package behavior.

### Desktop backend

- Create `apps/desktop/src-tauri/src/commands/recovery.rs`: singleton coordinator and generic Tauri commands.
- Modify `apps/desktop/src-tauri/src/commands/mod.rs`: export recovery commands.
- Modify `apps/desktop/src-tauri/src/commands/language_intelligence.rs`: expose all live server statuses to the coordinator.
- Modify `apps/desktop/src-tauri/src/commands/language_tools.rs`: share the existing installer with the coordinator and keep compatibility commands during migration.
- Modify `apps/desktop/src-tauri/src/language_server.rs`: return all session statuses without changing lifecycle ownership.
- Modify `apps/desktop/src-tauri/src/lib.rs`: register commands and focused command tests.

### Desktop frontend

- Create `apps/desktop/src/shared/api/recovery.ts`: TypeScript contracts, command wrappers, and event subscription.
- Modify `apps/desktop/src/shared/api/queries.ts`: add the recovery query key.
- Create `apps/desktop/src/features/health/RecoveryProvider.tsx`: one snapshot/event/selection controller.
- Create `apps/desktop/src/features/health/IDEHealthIndicator.tsx`: quiet aggregate titlebar button.
- Create `apps/desktop/src/features/health/IDEHealthPanel.tsx`: accessible health drawer and generic repair preview.
- Create `apps/desktop/src/features/health/health.css`: approved calm visual treatment.
- Modify `apps/desktop/src/app/main.tsx`: mount the provider inside QueryClient and Toast providers.
- Modify `apps/desktop/src/app/App.tsx`: render the aggregate indicator in the titlebar.
- Modify `apps/desktop/src/features/code/LanguageToolPill.tsx`: report capability use and open the shared health record.
- Modify `apps/desktop/src/features/code/LanguageToolPopover.tsx`: replace the local install/retry flow with `Review repair`/`Open IDE Health`.
- Modify `apps/desktop/src/features/code/useLiveLanguage.tsx`: send event-driven runtime observations and request refresh on live status changes.
- Modify `apps/desktop/src/features/code/language-tools.css`: keep contextual affordance compact and outside editor geometry.

### End-to-end coverage

- Modify `apps/desktop/e2e/fixtures.ts`: add a healthy empty recovery snapshot to the common fixture.
- Modify `apps/desktop/e2e/mock-ipc.ts`: add event emission support needed for recovery status updates.
- Create `apps/desktop/e2e/ide-health.spec.ts`: aggregate, diagnosis, confirmation, progress, verification failure, dismissal, and editor-availability scenarios.
- Modify `apps/desktop/e2e/language-tool-ui.spec.ts`: assert that the pill opens the shared record instead of owning a second installer UI.

---

### Task 0: Land the verified TypeScript compatibility prerequisite

**Files:**
- Modify: `crates/repodesk-core/src/language_tools.rs`
- Modify: `crates/repodesk-core/tests/language_tools_security.rs`
- Modify: `apps/desktop/e2e/language-tool-ui.spec.ts`

**Interfaces:**
- Consumes: existing `LanguageToolInstallService`, `managed_executable_path`, and managed install recipes.
- Produces: `typescript-language-server@5.3.0` paired with `typescript@6.0.3`; `managed_executable_path("typescript-language-server") -> None` for a stale incompatible companion package.

- [x] **Step 1: Review the prerequisite diff and confirm no recovery-platform code is mixed into it**

Run:

```bash
git diff -- crates/repodesk-core/src/language_tools.rs crates/repodesk-core/tests/language_tools_security.rs apps/desktop/e2e/language-tool-ui.spec.ts
```

Expected: only the TypeScript 6.0.3 pin, companion-version validation, matching Rust regression, and Playwright fixture revision/package updates.

- [x] **Step 2: Run the incompatible-install regression**

Run:

```bash
REPODESK_HOME=/tmp/repodesk-dev cargo test -p repodesk-core --test language_tools_security incompatible_managed_typescript_is_not_reported_ready -- --exact
```

Expected: PASS.

- [x] **Step 3: Run the language-tool UI regression file**

Run:

```bash
pnpm --dir apps/desktop exec playwright test e2e/language-tool-ui.spec.ts
```

Expected: all tests in the file pass.

- [x] **Step 4: Commit only the prerequisite**

```bash
git add crates/repodesk-core/src/language_tools.rs crates/repodesk-core/tests/language_tools_security.rs apps/desktop/e2e/language-tool-ui.spec.ts
git commit -m "fix: validate managed TypeScript runtime"
```

---

### Task 1: Add recovery contracts and deterministic record transitions

**Files:**
- Create: `crates/repodesk-core/src/recovery/mod.rs`
- Create: `crates/repodesk-core/src/recovery/types.rs`
- Create: `crates/repodesk-core/src/recovery/engine.rs`
- Create: `crates/repodesk-core/src/recovery/store.rs`
- Create: `crates/repodesk-core/tests/recovery_engine.rs`
- Create: `crates/repodesk-core/tests/recovery_store.rs`
- Modify: `crates/repodesk-core/src/lib.rs`

**Interfaces:**
- Consumes: `chrono::{DateTime, Utc}`, `serde::{Deserialize, Serialize}`.
- Produces: `RecoveryEngine::new(project: String, history_limit: usize)`, `observe(RecoveryObservation) -> ObserveOutcome`, `begin_repair(&str, &str, DateTime<Utc>) -> RepoDeskResult<RecoveryRecord>`, `finish_repair(&str, RepairCompletion) -> RepoDeskResult<RecoveryRecord>`, `prune_history(DateTime<Utc>)`, `snapshot() -> RecoverySnapshot`, `history() -> Vec<RecoveryAttempt>`, `RecoveryStore::load(&Path, String, usize)`, and `RecoveryStore::save(&Path, &RecoveryEngine)`.

- [x] **Step 1: Write failing public contract tests**

Create `crates/repodesk-core/tests/recovery_engine.rs` with these test cases and shared helpers:

```rust
use chrono::{TimeZone, Utc};
use repodesk_core::recovery::{
    RecoveryAction, RecoveryActionKind, RecoveryEngine, RecoveryFailureCode,
    RecoveryObservation, RecoverySeverity, RecoveryState, RepairCompletion,
};

fn at(second: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 10, 12, 0, second).unwrap()
}

fn missing_language(generation: u64) -> RecoveryObservation {
    RecoveryObservation {
        capability_id: "language:typescript-language-server".into(),
        module_id: "language_intelligence".into(),
        generation,
        observed_at: at(generation as u32),
        state: RecoveryState::NeedsApproval,
        severity: RecoverySeverity::Warning,
        code: Some(RecoveryFailureCode::MissingExecutable),
        title: "TypeScript intelligence is unavailable".into(),
        explanation: "The configured language server was not found.".into(),
        affected: vec!["Hover".into(), "Definitions".into()],
        unaffected: vec!["Editing".into(), "Save".into()],
        evidence: vec![],
        actions: vec![RecoveryAction {
            id: "install-managed-language-server".into(),
            label: "Review repair".into(),
            kind: RecoveryActionKind::Confirmable,
            recipe_id: Some("typescript-language-server".into()),
        }],
    }
}

#[test]
fn stale_observation_cannot_overwrite_newer_health() {
    let mut engine = RecoveryEngine::new("RepoDesk".into(), 100);
    engine.observe(missing_language(2));
    let mut stale = missing_language(1);
    stale.state = RecoveryState::Healthy;
    engine.observe(stale);
    assert_eq!(engine.snapshot().records[0].state, RecoveryState::NeedsApproval);
}

#[test]
fn failed_verification_never_becomes_healthy() {
    let mut engine = RecoveryEngine::new("RepoDesk".into(), 100);
    engine.observe(missing_language(1));
    engine.begin_repair("language:typescript-language-server", "install-managed-language-server", at(2)).unwrap();
    engine.finish_repair(
        "language:typescript-language-server",
        RepairCompletion::VerificationFailed {
            finished_at: at(3),
            summary: "Server installed but initialization failed".into(),
        },
    ).unwrap();
    assert_eq!(engine.snapshot().records[0].state, RecoveryState::Degraded);
}
```

Also add tests named `healthy_records_are_not_actionable`, `automatic_attempt_budget_stops_after_two_failures`, `history_is_bounded`, and `unknown_failure_has_no_guessed_recipe`.

- [x] **Step 2: Run the test target and verify the missing module failure**

Run:

```bash
REPODESK_HOME=/tmp/repodesk-dev cargo test -p repodesk-core --test recovery_engine
```

Expected: FAIL because `repodesk_core::recovery` does not exist.

- [x] **Step 3: Add the serialized contracts**

Create `types.rs` with these exact public names:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryState { Healthy, Degraded, Repairing, NeedsApproval, Blocked, Unknown }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoverySeverity { Info, Warning, Error }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryFailureCode {
    MissingExecutable,
    IncompatibleVersion,
    ProcessCrashed,
    InitializationFailed,
    RequestTimedOut,
    InvalidConfiguration,
    PermissionDenied,
    UnknownFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryActionKind { Automatic, Confirmable, Manual }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryAction {
    pub id: String,
    pub label: String,
    pub kind: RecoveryActionKind,
    pub recipe_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryEvidence {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryObservation {
    pub capability_id: String,
    pub module_id: String,
    pub generation: u64,
    pub observed_at: DateTime<Utc>,
    pub state: RecoveryState,
    pub severity: RecoverySeverity,
    pub code: Option<RecoveryFailureCode>,
    pub title: String,
    pub explanation: String,
    pub affected: Vec<String>,
    pub unaffected: Vec<String>,
    pub evidence: Vec<RecoveryEvidence>,
    pub actions: Vec<RecoveryAction>,
}
```

Define the remaining contracts exactly as follows:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryRecord {
    pub capability_id: String,
    pub module_id: String,
    pub generation: u64,
    pub diagnosis_revision: String,
    pub observed_at: DateTime<Utc>,
    pub state: RecoveryState,
    pub severity: RecoverySeverity,
    pub code: Option<RecoveryFailureCode>,
    pub title: String,
    pub explanation: String,
    pub affected: Vec<String>,
    pub unaffected: Vec<String>,
    pub evidence: Vec<RecoveryEvidence>,
    pub actions: Vec<RecoveryAction>,
    pub automatic_attempts: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverySnapshot {
    pub project: String,
    pub records: Vec<RecoveryRecord>,
    pub actionable_count: usize,
    pub warnings: Vec<String>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAttemptResult { Verified, Failed, VerificationFailed, Cancelled }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryAttempt {
    pub id: String,
    pub capability_id: String,
    pub diagnosis_revision: String,
    pub action_id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub result: Option<RecoveryAttemptResult>,
    pub verification_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveOutcome { Applied(RecoveryRecord), IgnoredStale }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepairCompletion {
    Verified { finished_at: DateTime<Utc>, summary: String },
    Failed { finished_at: DateTime<Utc>, summary: String },
    VerificationFailed { finished_at: DateTime<Utc>, summary: String },
    Cancelled { finished_at: DateTime<Utc>, summary: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryRisk { Low, Moderate, High }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryRepairPreview {
    pub capability_id: String,
    pub diagnosis_revision: String,
    pub action_id: String,
    pub title: String,
    pub summary: String,
    pub risk: RecoveryRisk,
    pub recipe_id: String,
    pub recipe_revision: String,
    pub changes: Vec<String>,
    pub network_required: bool,
    pub verification: String,
    pub confirmation_token: String,
    pub expires_at: DateTime<Utc>,
}
```

- [x] **Step 4: Implement the minimum deterministic engine**

Implement `RecoveryEngine` with `BTreeMap<String, RecoveryRecord>` and `VecDeque<RecoveryAttempt>`. `observe` ignores lower generations, derives `diagnosis_revision` from capability/generation/code, and resets the automatic-attempt count only when that revision changes. `begin_repair` validates the advertised action. `finish_repair` sets `healthy` only for `RepairCompletion::Verified` and records every completion in bounded history.

- [x] **Step 5: Export the module and run focused tests**

Add `store.rs` with an internal serialized state version `1`. `save` writes a sibling staging file, flushes it, and renames it over `recovery-state.json`; `load` rejects an unknown version or corrupt JSON and never silently discards it. `prune_history` removes attempts finished before the supplied cutoff before enforcing the count limit. Add tests proving records/history survive reload, retention is the newest 100 attempts within 30 days, and corrupt JSON returns an error while a fresh in-memory engine can still be created by the caller.

Add `pub mod recovery;` to `crates/repodesk-core/src/lib.rs`, re-export public contracts from `recovery/mod.rs`, then run:

```bash
REPODESK_HOME=/tmp/repodesk-dev cargo test -p repodesk-core --test recovery_engine --test recovery_store
cargo clippy -p repodesk-core --tests -- -D warnings
```

Expected: PASS with no warnings.

- [x] **Step 6: Commit the recovery domain**

```bash
git add crates/repodesk-core/src/lib.rs crates/repodesk-core/src/recovery crates/repodesk-core/tests/recovery_engine.rs crates/repodesk-core/tests/recovery_store.rs
git commit -m "feat: add recovery engine contracts"
```

---

### Task 2: Add the language recovery adapter and observable install progress

**Files:**
- Create: `crates/repodesk-core/src/recovery/language.rs`
- Create: `crates/repodesk-core/tests/language_recovery.rs`
- Modify: `crates/repodesk-core/src/recovery/mod.rs`
- Modify: `crates/repodesk-core/src/language_tools.rs`
- Modify: `crates/repodesk-core/tests/language_tools_security.rs`

**Interfaces:**
- Consumes: `LanguageServerDescriptor`, `LanguageServerAvailability`, `LanguageServerProfileState`, existing `LanguageToolInstallStatus`, and Task 1 `RecoveryObservation`.
- Produces: `language_observation(LanguageRecoveryInput) -> Option<RecoveryObservation>` and `LanguageToolInstallService::install_with_observer(token, observer) -> RepoDeskResult<LanguageToolInstallResult>`.

- [ ] **Step 1: Write failing adapter tests**

Create a concrete descriptor helper and tests covering these exact outcomes:

```rust
use chrono::{TimeZone, Utc};
use repodesk_core::language_intelligence::{
    LanguageServerAvailability, LanguageServerCapabilities,
    LanguageServerDescriptor, LanguageServerInitializationProfile,
    LanguageServerProfileState,
};
use repodesk_core::language_tools::{LanguageToolInstallState, LanguageToolInstallStatus};
use repodesk_core::recovery::{
    language_observation, LanguageRecoveryInput, LanguageRuntimeState,
    RecoveryActionKind, RecoveryFailureCode, RecoveryState,
};

fn at(second: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 10, 12, 0, second).unwrap()
}

fn typescript_descriptor(
    availability: LanguageServerAvailability,
    profile_state: LanguageServerProfileState,
) -> LanguageServerDescriptor {
    LanguageServerDescriptor {
        id: "typescript-language-server".into(),
        label: "TypeScript Language Server".into(),
        executable: "typescript-language-server".into(),
        arguments: vec!["--stdio".into()],
        languages: vec!["typescript".into(), "javascript".into()],
        availability,
        source: None,
        capabilities: LanguageServerCapabilities {
            diagnostics: true,
            hover: true,
            definition: true,
            references: true,
            completion: true,
            rename: true,
            formatting: true,
            document_symbols: true,
        },
        profile_state,
        initialization_profile: LanguageServerInitializationProfile::Default,
        install_recipe_id: Some("typescript-language-server".into()),
    }
}

#[test]
fn missing_active_server_requires_approved_managed_install() {
    let descriptor = typescript_descriptor(
        LanguageServerAvailability::Missing,
        LanguageServerProfileState::Active,
    );
    let observation = language_observation(LanguageRecoveryInput {
        project: "RepoDesk",
        generation: 1,
        descriptor: &descriptor,
        runtime: LanguageRuntimeState::NotStarted,
        runtime_error: None,
        install: None,
        observed_at: at(1),
    }).unwrap();
    assert_eq!(observation.state, RecoveryState::NeedsApproval);
    assert_eq!(observation.code, Some(RecoveryFailureCode::MissingExecutable));
    assert_eq!(observation.actions[0].kind, RecoveryActionKind::Confirmable);
    assert!(observation.unaffected.contains(&"Editing".to_string()));
}

#[test]
fn initialized_server_is_healthy() {
    let descriptor = typescript_descriptor(
        LanguageServerAvailability::Available,
        LanguageServerProfileState::Active,
    );
    let observation = language_observation(LanguageRecoveryInput {
        project: "RepoDesk",
        generation: 2,
        descriptor: &descriptor,
        runtime: LanguageRuntimeState::Ready,
        runtime_error: None,
        install: None,
        observed_at: at(2),
    }).unwrap();
    assert_eq!(observation.state, RecoveryState::Healthy);
    assert!(observation.actions.is_empty());
}

#[test]
fn initialization_error_is_degraded_and_restart_is_automatic() {
    let descriptor = typescript_descriptor(
        LanguageServerAvailability::Available,
        LanguageServerProfileState::Active,
    );
    let observation = language_observation(LanguageRecoveryInput {
        project: "RepoDesk",
        generation: 3,
        descriptor: &descriptor,
        runtime: LanguageRuntimeState::Error,
        runtime_error: Some("initialize request timed out"),
        install: None,
        observed_at: at(3),
    }).unwrap();
    assert_eq!(observation.state, RecoveryState::Degraded);
    assert_eq!(observation.code, Some(RecoveryFailureCode::InitializationFailed));
    assert_eq!(observation.actions[0].kind, RecoveryActionKind::Automatic);
    assert_eq!(observation.actions[0].id, "restart-language-session");
}

#[test]
fn discovery_only_profile_does_not_create_actionable_record() {
    let descriptor = typescript_descriptor(
        LanguageServerAvailability::Missing,
        LanguageServerProfileState::DiscoveryOnly,
    );
    assert!(language_observation(LanguageRecoveryInput {
        project: "RepoDesk",
        generation: 4,
        descriptor: &descriptor,
        runtime: LanguageRuntimeState::NotStarted,
        runtime_error: None,
        install: None,
        observed_at: at(4),
    }).is_none());
}

#[test]
fn installer_success_stays_repairing_until_runtime_verification() {
    let descriptor = typescript_descriptor(
        LanguageServerAvailability::Available,
        LanguageServerProfileState::Active,
    );
    let install = LanguageToolInstallStatus {
        recipe_id: "typescript-language-server".into(),
        state: LanguageToolInstallState::Ready,
        progress: 100,
        message: "Language server installed and version-probed".into(),
        started_at: at(4),
        finished_at: Some(at(5)),
        error: None,
    };
    let observation = language_observation(LanguageRecoveryInput {
        project: "RepoDesk",
        generation: 5,
        descriptor: &descriptor,
        runtime: LanguageRuntimeState::Starting,
        runtime_error: None,
        install: Some(&install),
        observed_at: at(5),
    }).unwrap();
    assert_eq!(observation.state, RecoveryState::Repairing);
}
```

- [ ] **Step 2: Run the adapter tests and verify failure**

Run:

```bash
REPODESK_HOME=/tmp/repodesk-dev cargo test -p repodesk-core --test language_recovery
```

Expected: FAIL because `recovery::language` is absent.

- [ ] **Step 3: Implement neutral runtime input and pure mapping**

Define:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageRuntimeState { NotStarted, Starting, Ready, Error }

pub struct LanguageRecoveryInput<'a> {
    pub project: &'a str,
    pub generation: u64,
    pub descriptor: &'a LanguageServerDescriptor,
    pub runtime: LanguageRuntimeState,
    pub runtime_error: Option<&'a str>,
    pub install: Option<&'a LanguageToolInstallStatus>,
    pub observed_at: DateTime<Utc>,
}
```

`language_observation` returns `None` for discovery-only profiles. Missing active profiles expose only a confirmable managed-install action. Runtime initialization errors expose an automatic `restart-language-session` action. A ready install status without a ready initialized runtime remains `repairing`, never `healthy`.

- [ ] **Step 4: Write a failing install-observer test**

Extend `language_tools_security.rs` with a fake runner and observer vector. Assert the observed state sequence is exactly `Installing` at progress 10, 30, 70, 90, then `Ready` at 100 for a successful managed install, and that the existing no-repository-write assertion remains true.

- [ ] **Step 5: Implement status observation without changing existing callers**

Add:

```rust
pub fn install_with_observer<F>(
    &self,
    confirmation_token: &str,
    observer: F,
) -> RepoDeskResult<LanguageToolInstallResult>
where
    F: Fn(&LanguageToolInstallStatus),
```

Keep `install()` as `self.install_with_observer(confirmation_token, |_| {})`. Thread `&dyn Fn(&LanguageToolInstallStatus)` through `execute_pending` and invoke it immediately after every successful `set_status`. Do not add a timer, polling loop, or mutable global callback.

- [ ] **Step 6: Run adapter and installer tests**

```bash
REPODESK_HOME=/tmp/repodesk-dev cargo test -p repodesk-core --test language_recovery
REPODESK_HOME=/tmp/repodesk-dev cargo test -p repodesk-core --test language_tools_security
cargo clippy -p repodesk-core --tests -- -D warnings
```

Expected: PASS with the exact progress sequence and existing security tests green.

- [ ] **Step 7: Commit the language adapter**

```bash
git add crates/repodesk-core/src/recovery crates/repodesk-core/src/language_tools.rs crates/repodesk-core/tests/language_recovery.rs crates/repodesk-core/tests/language_tools_security.rs
git commit -m "feat: diagnose language recovery states"
```

---

### Task 3: Expose a generic recovery coordinator through Tauri

**Files:**
- Create: `apps/desktop/src-tauri/src/commands/recovery.rs`
- Modify: `apps/desktop/src-tauri/src/commands/mod.rs`
- Modify: `apps/desktop/src-tauri/src/commands/language_intelligence.rs`
- Modify: `apps/desktop/src-tauri/src/commands/language_tools.rs`
- Modify: `apps/desktop/src-tauri/src/language_server.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: Task 1 `RecoveryEngine`; Task 2 `language_observation`; existing active language snapshot, session manager, and `LanguageToolInstallService`.
- Produces Tauri commands: `recovery_snapshot`, `recovery_check`, `recovery_repair_preview`, `recovery_repair_confirm`, `recovery_repair_cancel`, and `recovery_history`; emits `recovery-record-changed` carrying a `RecoveryRecord`.

- [ ] **Step 1: Write failing coordinator unit tests**

In `commands/recovery.rs`, add tests around an injected fake `RecoveryLanguageServices` trait:

```rust
trait RecoveryLanguageServices: Send + Sync {
    fn discovery(&self) -> Result<LanguageIntelligenceSnapshot, String>;
    fn statuses(&self) -> Vec<LanguageRuntimeStatus>;
    fn install_status(&self, recipe_id: &str) -> Result<Option<LanguageToolInstallStatus>, String>;
    fn preview(&self, recipe_id: &str) -> Result<LanguageToolInstallPreview, String>;
    fn install_observed(
        &self,
        token: &str,
        observer: &dyn Fn(&LanguageToolInstallStatus),
    ) -> Result<LanguageToolInstallResult, String>;
    fn cancel(&self, recipe_id: &str) -> Result<bool, String>;
    fn restart(&self, server_id: &str) -> Result<(), String>;
}
```

Test: first capability check creates one missing TypeScript record; snapshot alone does not invent unused records; preview returns an outer token bound to capability and diagnosis revision; a changed generation rejects that token; installer `Ready` remains `repairing` until a ready runtime observation verifies it; emitted updates never include raw installer output. Also assert that an initialization failure automatically invokes `restart` at most twice for one diagnosis revision and that the third check returns a non-automatic action without invoking it again. Reload a coordinator from a temporary RepoDesk home and assert its latest record and bounded attempt history survive.

- [ ] **Step 2: Run the desktop library test and verify failure**

Run:

```bash
REPODESK_HOME=/tmp/repodesk-dev cargo test -p repodesk-desktop recovery --lib
```

Expected: FAIL because the recovery command module does not exist.

- [ ] **Step 3: Add all-session status access**

Add `LanguageServerManager::statuses() -> Vec<LanguageServerStatus>` by cloning every current session status under the registry lock. Keep `status()` for compatibility. Add a unit test with two session registry entries proving both project/server keys remain distinguishable.

- [ ] **Step 4: Share existing language services without adding a second installer**

Make the existing installer `pub(crate)` in `commands/language_tools.rs`. In `commands/language_intelligence.rs`, expose crate-private functions returning all statuses and restarting one server by id. The recovery coordinator must depend on these existing owners; it must not create a second `LanguageServerManager` or `LanguageToolInstallService`.

- [ ] **Step 5: Implement coordinator records and bound confirmations**

Use:

```rust
pub struct RecoveryCoordinator {
    engine: Mutex<RecoveryEngine>,
    pending: Mutex<HashMap<String, PendingRecoveryRepair>>,
    services: Arc<dyn RecoveryLanguageServices>,
    sequence: AtomicU64,
}

struct PendingRecoveryRepair {
    capability_id: String,
    diagnosis_revision: String,
    action_id: String,
    recipe_id: String,
    adapter_confirmation_token: String,
    expires_at: DateTime<Utc>,
}
```

Create the outer token with SHA-256 over project identity, capability id, diagnosis revision, action id, recipe revision, expiry, and a monotonic sequence. Return a generic `RecoveryRepairPreview` that contains display scope and the outer token but not the adapter token. Confirm removes the outer token before use, rechecks project and diagnosis revision, starts the engine attempt, streams bounded progress records, and completes only after a fresh live-runtime check reports `Ready`. During `recovery_check`, run an advertised automatic restart immediately only while `RecoveryEngine` reports remaining automatic-attempt budget; observe and emit its post-restart verification result before returning.

- [ ] **Step 6: Register the generic commands and event**

Command signatures:

```rust
#[tauri::command]
pub fn recovery_snapshot() -> Result<RecoverySnapshot, String>;

#[tauri::command]
pub fn recovery_check(app: AppHandle, capability_id: String) -> Result<RecoveryRecord, String>;

#[tauri::command]
pub fn recovery_repair_preview(capability_id: String, action_id: String) -> Result<RecoveryRepairPreview, String>;

#[tauri::command]
pub async fn recovery_repair_confirm(app: AppHandle, confirmation_token: String) -> Result<RecoveryRecord, String>;

#[tauri::command]
pub fn recovery_repair_cancel(recipe_id: String) -> Result<bool, String>;

#[tauri::command]
pub fn recovery_history() -> Result<Vec<RecoveryAttempt>, String>;
```

Cancel and history commands accept only stable ids. Emit `recovery-record-changed` after observations and state transitions. Persist the engine after each applied observation and repair transition at `RepoDeskPaths::home/recovery/recovery-state.json`. If loading or saving fails, keep the current in-memory engine usable and add one bounded `history_unavailable` evidence item to the snapshot. Truncate/redact evidence using a dedicated function with a 2,000-character value limit and sensitive-key matching for token, authorization, password, secret, api-key, and home paths.

- [ ] **Step 7: Run backend tests and lint**

```bash
REPODESK_HOME=/tmp/repodesk-dev cargo test -p repodesk-desktop recovery --lib
REPODESK_HOME=/tmp/repodesk-dev cargo test -p repodesk-desktop language_server --lib
cargo clippy -p repodesk-desktop --all-targets --all-features -- -D warnings
```

Expected: PASS; no raw adapter token or output crosses the serialized preview/record boundary.

- [ ] **Step 8: Commit the backend coordinator**

```bash
git add apps/desktop/src-tauri/src/commands/recovery.rs apps/desktop/src-tauri/src/commands/mod.rs apps/desktop/src-tauri/src/commands/language_intelligence.rs apps/desktop/src-tauri/src/commands/language_tools.rs apps/desktop/src-tauri/src/language_server.rs apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/Cargo.toml Cargo.lock
git commit -m "feat: expose language recovery coordinator"
```

---

### Task 4: Add the shared recovery controller and minimum health surface

**Files:**
- Create: `apps/desktop/src/shared/api/recovery.ts`
- Create: `apps/desktop/src/features/health/RecoveryProvider.tsx`
- Create: `apps/desktop/src/features/health/IDEHealthIndicator.tsx`
- Create: `apps/desktop/src/features/health/IDEHealthPanel.tsx`
- Create: `apps/desktop/src/features/health/health.css`
- Modify: `apps/desktop/src/shared/api/queries.ts`
- Modify: `apps/desktop/src/app/main.tsx`
- Modify: `apps/desktop/src/app/App.tsx`
- Modify: `apps/desktop/e2e/fixtures.ts`
- Modify: `apps/desktop/e2e/mock-ipc.ts`
- Create: `apps/desktop/e2e/ide-health.spec.ts`

**Interfaces:**
- Consumes: Task 3 Tauri command/event JSON.
- Produces: `useRecovery()` with `snapshot`, `history`, `selected`, `openHealth(capabilityId?)`, `closeHealth()`, `check(capabilityId)`, `preview(capabilityId, actionId)`, `confirm(token)`, and `cancel(recipeId)`; a minimum accessible titlebar indicator and panel make the task independently testable.

- [ ] **Step 1: Add controlled mock event delivery**

Extend `mock-ipc.ts` so tests can call:

```ts
await emitMockTauriEvent(page, "recovery-record-changed", nextRecord);
```

Store registered callbacks by event name, invoke only current listeners, and remove them when Tauri calls `unregisterCallback`. Preserve existing command recording behavior.

- [ ] **Step 2: Write a failing controller-to-panel Playwright test**

Add these common fixtures to `onboardedFixtures`:

```ts
recovery_snapshot: {
  project: "RepoDesk",
  records: [],
  actionable_count: 0,
  warnings: [],
  generated_at: "2026-08-10T12:00:00Z",
},
recovery_history: [],
```

In `ide-health.spec.ts`, override the snapshot with one `needs_approval` TypeScript record, assert a titlebar button named `IDE health: 1 needs attention`, click it, assert the exact capability title in the `IDE Health` dialog, emit a newer `repairing` record, and assert the same selected card now says `Repairing` without another `recovery_snapshot` invocation.

- [ ] **Step 3: Define frontend contracts matching Rust exactly**

In `recovery.ts`, define snake-case-compatible fields and these unions:

```ts
export type RecoveryState = "healthy" | "degraded" | "repairing" | "needs_approval" | "blocked" | "unknown";
export type RecoverySeverity = "info" | "warning" | "error";
export type RecoveryActionKind = "automatic" | "confirmable" | "manual";
export const RECOVERY_CHANGED_EVENT = "recovery-record-changed";
export const RECOVERY_QUERY_KEY = ["recovery_snapshot"] as const;
```

Command wrappers must call `callCommand`, and `subscribeRecoveryChanges` must use Tauri `listen` and return its unlisten function. Define wrappers for snapshot, history, check, preview, confirm, and cancel. `RecoverySnapshot` includes `warnings: string[]`; the panel renders these as non-blocking notices.

- [ ] **Step 4: Implement the provider without polling**

Use one `useQuery` each for snapshot and bounded history, one event subscription that updates the matching record with `queryClient.setQueryData`, and local state only for panel visibility, selected capability, preview, and mutation progress. `check` explicitly invokes `recovery_check`; no `setInterval` is allowed. Ignore an event whose generation is lower than the cached record generation.

- [ ] **Step 5: Render a minimum accessible indicator and panel**

The indicator counts only `degraded`, `needs_approval`, and `blocked`. The panel uses `role="dialog"`, an `IDE Health` accessible name, a close button, the selected record title/state/explanation, and a polite progress region. It does not yet implement the final card hierarchy or repair-detail composition; Task 5 adds those against failing tests.

- [ ] **Step 6: Mount the provider and run the independent task gate**

Wrap `<App />` inside `<RecoveryProvider>` under `QueryClientProvider` and `ToastProvider` in `main.tsx`. Render the indicator next to Git status and render the panel from the provider.

Run:

```bash
pnpm --dir apps/desktop run build
pnpm --dir apps/desktop exec playwright test e2e/ide-health.spec.ts
```

Expected: the production build and controller-to-panel event test pass.

- [ ] **Step 7: Commit the shared controller and minimum surface**

```bash
git add apps/desktop/src/shared/api/recovery.ts apps/desktop/src/shared/api/queries.ts apps/desktop/src/features/health apps/desktop/src/app/main.tsx apps/desktop/src/app/App.tsx apps/desktop/e2e/fixtures.ts apps/desktop/e2e/mock-ipc.ts apps/desktop/e2e/ide-health.spec.ts
git commit -m "feat: add recovery health controller"
```

---

### Task 5: Build the IDE Health indicator and panel

**Files:**
- Modify: `apps/desktop/src/features/health/IDEHealthIndicator.tsx`
- Modify: `apps/desktop/src/features/health/IDEHealthPanel.tsx`
- Modify: `apps/desktop/src/features/health/health.css`
- Modify: `apps/desktop/src/features/health/RecoveryProvider.tsx`
- Modify: `apps/desktop/e2e/ide-health.spec.ts`

**Interfaces:**
- Consumes: Task 4 `useRecovery()`.
- Produces: a titlebar `IDE health: Healthy` or `IDE health: N need attention` button and an accessible `dialog` named `IDE Health`.

- [ ] **Step 1: Write failing aggregate and panel tests**

Add Playwright tests with concrete fixtures asserting:

- healthy or empty snapshots render `IDE health: Healthy` with no numeric badge;
- one degraded plus one needs-approval record renders count `2`;
- clicking the indicator opens `role=dialog` named `IDE Health`;
- healthy cards are collapsed and the selected problem is expanded;
- the selected problem shows affected/unaffected features, risk, and `Review repair`;
- recent repair history shows action, result, verification summary, and timestamp without raw output;
- Escape and the close button close the panel and restore focus to the indicator;
- keyboard Tab reaches every action in a stable order.

- [ ] **Step 2: Run the tests to verify UI absence**

```bash
pnpm --dir apps/desktop exec playwright test e2e/ide-health.spec.ts
```

Expected: FAIL because the minimum panel does not yet implement the approved hierarchy, history, focus restoration, or final layout.

- [ ] **Step 3: Implement the quiet aggregate indicator**

Count only `degraded`, `needs_approval`, and `blocked`. Render a small titlebar button next to Git status. Healthy state uses neutral styling; actionable state uses one amber accent and a count. Do not render per-module icons, animations, or a permanent green badge.

- [ ] **Step 4: Implement the accessible health panel**

Render module-grouped records, expanding `selected` first. The problem card order is `blocked`, `needs_approval`, `degraded`, `repairing`, `unknown`, `healthy`. Show exactly the approved four-part explanation: stopped feature, diagnosis, repair scope, verification. Healthy records remain one compact row. Show bounded recent history below the records. Use `aria-live="polite"` only for repair progress/result. Wrap the panel body in the existing `ErrorBoundary` with scope `ide-health` so a rendering failure does not take down the shell.

- [ ] **Step 5: Apply the approved visual direction**

Use existing foundation variables, 12–16px radii, one accent border on the selected card, restrained shadow on the panel, and no nested full-page scroll container. The panel owns one vertical scrollbar on the right. At widths below 760px it becomes full-width while preserving one scroll owner.

- [ ] **Step 6: Run UI gates**

```bash
pnpm --dir apps/desktop run build
pnpm --dir apps/desktop exec playwright test e2e/ide-health.spec.ts e2e/ui-audit.spec.ts
```

Expected: PASS with no horizontal overflow or duplicate vertical scrollbars.

- [ ] **Step 7: Commit IDE Health**

```bash
git add apps/desktop/src/features/health apps/desktop/e2e/ide-health.spec.ts
git commit -m "feat: add IDE Health center"
```

---

### Task 6: Route the editor’s language failure through IDE Health

**Files:**
- Modify: `apps/desktop/src/features/code/LanguageToolPill.tsx`
- Modify: `apps/desktop/src/features/code/LanguageToolPopover.tsx`
- Modify: `apps/desktop/src/features/code/useLiveLanguage.tsx`
- Modify: `apps/desktop/src/features/code/language-tools.css`
- Modify: `apps/desktop/e2e/language-tool-ui.spec.ts`
- Modify: `apps/desktop/e2e/ide-health.spec.ts`

**Interfaces:**
- Consumes: Task 4 `useRecovery()` and Task 3 capability ids `language:<server_id>`.
- Produces: one shared repair flow; the editor no longer owns installation preview/confirmation/progress state.

- [ ] **Step 1: Rewrite the language-tool UI tests to fail against the local dialog**

For missing TypeScript, assert:

```ts
await page.getByRole("button", { name: "TypeScript language tool: Missing" }).click();
await page.getByRole("button", { name: "Review repair" }).click();
await expect(page.getByRole("dialog", { name: "IDE Health" })).toBeVisible();
await expect(page.getByRole("dialog", { name: "Install TypeScript Language Server" })).toHaveCount(0);
```

Add assertions that a ready pill stays compact, a runtime error says editing remains available, dismiss hides only the contextual notice, and the same record remains in IDE Health.

- [ ] **Step 2: Run focused UI tests and verify failure**

```bash
pnpm --dir apps/desktop exec playwright test e2e/language-tool-ui.spec.ts e2e/ide-health.spec.ts
```

Expected: FAIL because the pill still owns the install flow.

- [ ] **Step 3: Remove duplicate repair ownership from the pill**

Delete its preview, install status, polling timer, confirmation, cancellation, and synthetic progress state. On first actionable state, call `check("language:" + server.id)` once per diagnosis-relevant server/project change. `Review repair` calls `openHealth(capabilityId)`.

- [ ] **Step 4: Feed live language events into recovery refresh**

When `useLiveLanguage` receives `language-server-status` or a command error for the active server, invoke the explicit recovery check for that capability. Coalesce by the provider/engine query path rather than a timer. Status changes for other project/server ids remain ignored.

- [ ] **Step 5: Complete generic preview, confirmation, and progress UX**

From IDE Health, `Review repair` calls `recovery_repair_preview`. Render exact package/version, destination, network use, writes, risk, and verifier. Confirm sends only `{ confirmationToken }`. `recovery-record-changed` drives progress. Failed verification remains degraded and offers `Check again`; it never displays success.

- [ ] **Step 6: Run editor geometry and repair tests**

```bash
pnpm --dir apps/desktop run build
pnpm --dir apps/desktop exec playwright test e2e/language-tool-ui.spec.ts e2e/ide-health.spec.ts e2e/editor-ui.spec.ts
```

Expected: PASS; CodeMirror retains one right-side scrollbar, full gutter background, visible final line, and clickable line numbers.

- [ ] **Step 7: Commit the contextual integration**

```bash
git add apps/desktop/src/features/code apps/desktop/src/features/health apps/desktop/e2e/language-tool-ui.spec.ts apps/desktop/e2e/ide-health.spec.ts
git commit -m "feat: connect language repair to IDE Health"
```

---

### Task 7: Harden failure containment, event cleanup, and final verification

**Files:**
- Modify: `apps/desktop/e2e/mock-ipc.ts`
- Modify: `apps/desktop/e2e/ide-health.spec.ts`

**Interfaces:**
- Consumes: complete language vertical slice.
- Produces: deterministic event-driven E2E coverage, recovery from panel rendering failure, and a green workspace handoff.

- [ ] **Step 1: Write listener cleanup and stale-event regressions**

Use the controlled mock event delivery from Task 4 to assert that closing/remounting the provider unregisters the old callback, one emitted event produces one UI update, and a lower-generation record cannot replace the current selected record.

- [ ] **Step 2: Add event and failure-containment tests**

Cover: an emitted progress record updates the open panel without another snapshot call; a stale generation is ignored; switching project resets selection and invalidates pending preview; unknown failure exposes bounded details but no guessed repair; IDE Health rendering failure is caught and can be reopened; editor typing and save remain available during every non-healthy state.

- [ ] **Step 3: Run formatting and focused backend tests**

```bash
cargo fmt --all -- --check
REPODESK_HOME=/tmp/repodesk-dev cargo test -p repodesk-core --test recovery_engine --test language_recovery --test language_tools_security
REPODESK_HOME=/tmp/repodesk-dev cargo test -p repodesk-desktop recovery --lib
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: every command exits 0 with no warnings.

- [ ] **Step 4: Run complete workspace and desktop gates**

```bash
REPODESK_HOME=/tmp/repodesk-dev cargo test --workspace
pnpm --dir apps/desktop run build
pnpm --dir apps/desktop exec playwright test
git diff --check
```

Expected: all Rust tests, production build, and complete Playwright suite pass; `git diff --check` emits nothing.

- [ ] **Step 5: Review scope and commit hardening**

Run:

```bash
git status --short
git diff --stat HEAD
git diff -- apps/desktop/e2e/mock-ipc.ts apps/desktop/e2e/ide-health.spec.ts
```

Expected: only event-cleanup and failure-containment test hardening remains uncommitted; unrelated user work is absent.

```bash
git add apps/desktop/e2e/mock-ipc.ts apps/desktop/e2e/ide-health.spec.ts
git commit -m "test: harden IDE recovery flow"
```

---

## Completion criteria

- Opening a supported file triggers one relevant language capability check without continuous polling.
- A compatible initialized server is healthy; a missing, incompatible, crashed, or initialization-failed server has a typed, plain-language diagnosis.
- The editor stays fully usable in every degraded state.
- The titlebar is quiet when healthy and opens one IDE Health panel for actionable records.
- Contextual `Review repair` selects the exact same backend record.
- Installation previews are diagnosis-bound; confirmation sends only an opaque token.
- Managed installation progress arrives through events, preserves the last-known-good installation, and never edits the repository.
- Installer success is not health success: live initialization must verify the repair.
- Unknown failures offer bounded redacted evidence and manual next steps, never generated shell commands.
- Latest records and bounded repair history survive application restart; storage failure leaves editing and in-memory health usable.
- Focused and full Rust, Clippy, frontend build, Playwright, and diff checks pass.
