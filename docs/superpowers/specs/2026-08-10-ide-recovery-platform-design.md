# IDE recovery platform

## Goal

Make RepoDesk recoverable by a normal user when an editor service, toolchain, task runner, Git operation, terminal, or local AI provider is unavailable or broken. The user should not need to understand logs, package-manager layouts, or language-server protocols to restore the feature safely.

The approved experience combines:

- a centralized, event-driven Recovery Engine in the backend;
- one calm `IDE Health` center for the complete workspace state;
- contextual `Repair` entry points beside the affected capability;
- automatic execution only for allowlisted, reversible local actions;
- explicit confirmation for installs, downgrades, project changes, or other material mutations;
- mandatory verification after every attempted repair.

## Product principles

1. Editing remains available. A failed optional service degrades its own capability instead of blocking the editor or application.
2. Explain before mutating. RepoDesk states what failed, what the repair changes, where it writes, whether it needs the network, and how it will verify success.
3. Prefer a working last-known-good tool. A failed upgrade or staged installation cannot replace a verified installation.
4. One diagnosis vocabulary. Editor, Git, terminal, task, and AI surfaces consume the same typed recovery records rather than inventing separate error UX.
5. No continuous polling. Checks run on relevant events and explicit user refreshes.
6. Evidence is useful but bounded. Logs are redacted, size-limited, and hidden behind details by default.

## First release scope

The first complete slice covers:

- language servers and RepoDesk-managed language tools;
- Rust and Node-based toolchains required by supported language profiles;
- project task runners used for build, test, and lint actions;
- Git repository discovery and common local Git failures;
- the integrated terminal and its shell process;
- configured local AI providers and their selected models.

Remote Git hosting, cloud AI account billing, arbitrary user-authored repair scripts, plugin installation, operating-system package managers, and automatic project-file edits remain outside this slice.

## Architecture

### Recovery Engine

`repodesk-core` owns a `recovery` module that is independent of Tauri and React. It contains the registry, orchestration policy, typed records, repair authorization, verification, bounded evidence, and repair history.

Each supported subsystem registers a `RecoveryAdapter` with:

- a stable module and capability id;
- event types that can trigger its probes;
- one or more read-only probes;
- typed diagnosis mapping;
- zero or more allowlisted repair recipes;
- a post-repair verifier;
- sensitivity rules for evidence and user-visible details.

Adapters do not render UI and never accept arbitrary commands from the frontend. Existing language-tool installation, workflow doctor, runtime discovery, model health, Git, and terminal services become adapter dependencies rather than being duplicated.

The Tauri layer exposes snapshots, event subscriptions, repair previews, confirmations, execution status, cancellation when supported, and repair history. React Query holds the current projection; Tauri events update only affected records.

### Event-driven checks

The engine schedules probes only after:

- application startup;
- project open, close, or root change;
- first use of a capability during the session;
- a subsystem lifecycle event such as process exit or request timeout;
- a failed user action attributable to a registered capability;
- completion or failure of a repair;
- an explicit `Check again` action.

Identical pending checks are coalesced by `(project, capability, probe revision)`. Each probe has a timeout and short cooldown to prevent failure loops. Opening IDE Health reads the latest snapshot; it does not cause all network-aware probes to run automatically.

### State model

Every capability projects one of these user-facing states:

- `healthy`: verified and available;
- `degraded`: core work continues but one or more optional features are unavailable;
- `repairing`: an approved recipe is running;
- `needs_approval`: a recommended repair exists but requires confirmation;
- `blocked`: RepoDesk cannot repair safely and provides a concrete manual next step;
- `unknown`: not checked in the current project context.

Transient internal phases such as `checking` and `verifying` may be shown as activity without replacing the last known health result. A crash never erases the last bounded evidence or the last-known-good executable.

### Typed diagnosis

A diagnosis contains:

- stable `code`, `module_id`, and `capability_id`;
- severity and current state;
- short plain-language title and explanation;
- affected and unaffected functionality;
- evidence references rather than unbounded raw output;
- available repair recipe ids and the recommended recipe;
- detection timestamp, project identity, and adapter revision;
- correlation id connecting the probe, repair attempt, and verification result.

Initial failure families include `missing_executable`, `incompatible_version`, `process_crashed`, `initialization_failed`, `request_timed_out`, `invalid_configuration`, `permission_denied`, `repository_unavailable`, `shell_unavailable`, `provider_unreachable`, `model_missing`, and `unknown_failure`.

Unknown failures are never converted into guessed install commands. They remain degraded or blocked, expose redacted evidence, and offer `Check again`, `Copy diagnostic report`, and the relevant manual guidance.

## Repair policy

### Safe automatic repairs

An action may run without a new confirmation only when its compiled recipe declares that it is local, reversible, bounded, and non-project-mutating. The first release permits only:

- restart a RepoDesk-owned subprocess;
- reinitialize a session after a confirmed configuration or executable change;
- clear a RepoDesk-owned ephemeral cache with a defined byte and path boundary;
- discard an incomplete staging installation;
- remove stale RepoDesk-owned session metadata and rerun its read-only probe.

Automatic repair has a strict attempt budget. Two failed automatic attempts for the same diagnosis revision suppress further automation and move the record to `needs_approval` or `blocked`.

### Confirmation-required repairs

The user must approve:

- installing, upgrading, or downgrading a managed tool;
- downloading files or accessing a package registry;
- changing project files, manifests, lockfiles, or configuration;
- removing anything outside a RepoDesk-owned staging/cache directory;
- changing an AI provider endpoint, selected model, or credentials;
- executing a command in the project or user shell.

The preview shows exact scope, destination, source ecosystem, pinned version where applicable, network use, estimated reversibility, affected feature, verification step, and rollback behavior. A confirmation token is bound to the diagnosis, recipe id, recipe revision, project identity, and preview digest. A stale token is rejected.

### Execution and rollback

Repairs use structured argv and existing typed service APIs without a shell. Only compiled first-party recipe ids are accepted. Installations keep the existing staging and atomic-promotion boundary: the active tool changes only after its probe succeeds. Configuration changes, if added in a later release, require a backup and explicit rollback recipe before they can become confirmable.

The verifier is part of the recipe, not an optional follow-up. A zero process exit is insufficient. For example, a language server must complete protocol initialization; a task runner must pass its discovery probe; Git must reopen the intended repository; a provider must answer the configured health/model request.

## IDE Health experience

### Global entry point

The application shell exposes a small aggregate health indicator. It stays visually quiet when all checked capabilities are healthy and gains a count only for `degraded`, `needs_approval`, or `blocked` records. `Unknown` does not appear as a warning until the capability is first needed.

Opening `IDE Health` shows:

- an aggregate status and last event-driven check time;
- cards grouped by Language Intelligence, Toolchains, Tasks, Git, Terminal, and Local AI;
- the short diagnosis and affected feature;
- the recommended action with risk label;
- `Repair`, `Review repair`, `Check again`, or `Show manual steps` according to policy;
- expandable evidence and a redacted copyable report;
- recent repair history and verification outcomes.

Healthy cards remain compact. The current problem expands first. The UI does not imitate a monitoring dashboard and does not show constant spinners or meaningless green checks.

### Contextual entry points

The affected feature shows one small status affordance using the same recovery record:

- editor status/pill for language intelligence;
- task result for task-runner failures;
- source-control surface for Git;
- terminal status for shell/PTY failures;
- model selector or AI action for provider/model failures.

The contextual message states what remains usable. Its primary action opens the matching record or repair preview in IDE Health; it does not implement a second repair flow. Dismissal hides the contextual notice for the current diagnosis revision but never removes the IDE Health record.

### Repair details and history

Before confirmation, the details view separates four questions:

1. What stopped working?
2. Why does RepoDesk think this happened?
3. What exactly will the repair change?
4. How will RepoDesk prove it works afterward?

History stores the recipe id/revision, result, timestamps, verification summary, and bounded evidence references. It never stores credentials, environment dumps, full command output, or project source. A user can copy a sanitized diagnostic bundle for support.

## Adapter behavior

### Language servers and toolchains

The adapter builds on the language registry, session lifecycle, and managed installer. It distinguishes executable absence, incompatible companion packages, version-probe failure, initialization failure, crash, request timeout, and unsupported profiles. Successful verification requires a real initialized session and advertised capabilities, not only `--version`.

Managed repair never modifies project dependencies. Rustup changes and all package downloads require confirmation. A failed server leaves syntax highlighting, editing, scrolling, selection, and save available.

### Task runners

The adapter discovers only task systems already supported by RepoDesk. Missing executables, invalid task definitions, permission errors, and non-zero task results remain distinct. A failing project test is a task result, not an IDE health failure; health becomes degraded only when RepoDesk cannot invoke or supervise the runner itself.

RepoDesk never automatically changes a task or installs its project dependency. Manual steps may be offered as copyable text after the engine confirms that no allowlisted repair applies.

### Git

The adapter checks repository accessibility, working-directory identity, executable availability, and operation-specific failures. Merge conflicts, rejected pushes, authentication, and repository corruption are not auto-repaired. Contextual guidance explains the category and preserves user changes. Any future mutating recipe must be separately designed and cannot enter the generic first-release allowlist.

### Terminal

The adapter checks the selected shell path, PTY startup, process exit, and working-directory access. It may automatically restart only a RepoDesk-owned terminal session with no running foreground process. Otherwise the user approves closing/restarting the session. RepoDesk does not edit shell profiles or infer shell commands.

### Local AI providers

The adapter reuses configured provider health and model discovery. Disabled providers do not cause network probes. Unreachable endpoint, authentication failure, missing model, incompatible API, and timeout remain distinct. Retrying a read-only health request is safe; downloading a model or changing configuration requires confirmation and a dedicated recipe not included in the first implementation slice.

## Concurrency and failure containment

- Probes and repairs are keyed by project and capability; unrelated modules run independently.
- Only one mutating repair may run for a capability at a time.
- Closing or switching a project cancels project-scoped probes and invalidates pending confirmation tokens.
- Late probe or repair events carry a generation id and cannot overwrite a newer diagnosis.
- Engine storage failure does not block editing; the current session continues with an in-memory snapshot and reports that history is unavailable.
- A frontend rendering failure is contained by the existing tab/application error boundaries. Reopening IDE Health reads the backend snapshot.
- Bounded stderr and command output are redacted before crossing the Tauri boundary.

## Persistence

Persist only the latest record per capability and a bounded repair ledger under the RepoDesk application-data root. Records include schema and adapter revisions so stale diagnoses can become `unknown` after an upgrade. Default retention is the latest 100 repair attempts or 30 days, whichever is smaller. Staging data, cache data, evidence, and history have separate explicit directories and cleanup rules.

No recovery state is written into the active repository or included in Git. Project identity uses the existing canonical project identity rather than exposing absolute paths in user-copyable reports.

## Testing

### Core contracts

- registry rejects duplicate ids, unsafe recipe classes, recipes without verifiers, and invalid automatic-repair declarations;
- state transitions cover healthy, degraded, repairing, needs-approval, blocked, unknown, stale generations, cancellation, and failed verification;
- event coalescing, timeouts, cooldowns, and attempt budgets are deterministic under a fake clock;
- confirmation tokens reject changed diagnosis, scope, recipe revision, project identity, or preview digest;
- evidence limits and redaction cover environment secrets, credentials, home paths, URLs with tokens, and oversized output;
- persistence covers schema migration, corruption fallback, retention, and in-memory continuation.

### Adapter contracts

Every adapter runs against a shared conformance suite proving that probes are read-only, repairs are allowlisted, automatic recipes respect policy, verification is mandatory, and cancellation/stale results cannot corrupt state.

Focused tests cover:

- real protocol initialization for compatible language-server fixtures and incompatible companion-package regression cases;
- tool installation staging, atomic promotion, last-known-good preservation, and downgrade confirmation;
- distinction between a task failure and a broken task runner;
- Git conflicts/authentication as guided blocked states with no mutation;
- terminal restart refusal while a foreground process is active;
- disabled AI providers avoiding network access and model-missing diagnoses remaining actionable.

### Frontend and end to end

- aggregate indicator remains quiet when healthy and counts only actionable records;
- IDE Health groups records, expands the current problem, and exposes accessible keyboard/screen-reader states;
- contextual repair opens the exact shared record and dismissal is scoped to the diagnosis revision;
- preview displays all mutation and verification fields before confirmation;
- progress, cancellation, success, failed verification, manual guidance, and repair history are covered;
- editing remains functional while language intelligence is missing, crashed, repairing, or blocked;
- app/tab ErrorBoundary recovery still works when IDE Health itself throws;
- no second scrollbar, gutter clipping, or editor overlay regression is introduced.

## Delivery slices

1. **Recovery contracts and language vertical slice**: core registry/state machine, snapshot/events, IDE Health shell, contextual language entry, existing managed-installer integration, verification, and history.
2. **Developer runtime adapters**: toolchain and task-runner diagnoses using the same contracts and UI.
3. **Workspace adapters**: Git and terminal probes with conservative repair policy.
4. **Local AI adapter**: provider/model health projection and safe retry behavior.
5. **Supportability hardening**: sanitized diagnostic bundles, retention controls, accessibility, performance, and full cross-adapter conformance suite.

Each slice must leave the application usable when its adapter is unavailable and must pass its targeted tests plus the existing workspace, Clippy, desktop build, and Playwright gates.

## Future extensions

The centralized contracts deliberately enable later modules without granting them automatic authority:

- a signed first-party recovery-recipe catalog with explicit versioning;
- an incident timeline that correlates a failing task, language-server crash, and recent tool change without collecting source code;
- a reproducible `Recovery Capsule` containing redacted diagnoses and environment fingerprints for support or teammate handoff;
- preflight health profiles for a repository that explain missing prerequisites before the first build;
- opt-in plugin adapters whose capabilities and mutation classes are visible and independently permissioned.

These extensions reuse the registry, evidence, authorization, and verification contracts. They are not part of the first implementation plan.

## Out of scope

- arbitrary shell commands generated from an error message;
- automatic operating-system package installation;
- automatic edits to source, manifests, lockfiles, Git history, credentials, or shell profiles;
- treating test failures, compiler errors, merge conflicts, or rejected pushes as automatically repairable IDE failures;
- continuous background polling or telemetry uploads;
- enabling third-party repair plugins before a separate capability and trust-boundary design;
- guaranteeing recovery from unknown failures without user-visible evidence and a verifiable recipe.
