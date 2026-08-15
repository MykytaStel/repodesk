# Credential Single-Owner Implementation Plan

Date: 2026-08-15
Design: `docs/superpowers/specs/2026-08-15-credential-single-owner-design.md`
Branch: `refactor/credential-single-owner`

## Objective

Converge provider credentials onto one security boundary: the Settings credential editor is the only user-facing mutation owner; `credential_set` / `credential_delete` are the only user-triggered secret writers; OS keychain is canonical writable storage; environment variables are read-only fallback; generic provider preferences are non-secret by type and command contract.

The change must also remove the current ambiguity where effective environment credentials look like stored keychain credentials, prevent quota/preference updates from passing through a secret-bearing settings command, and keep model-health/auth diagnostics fresh after credential mutations.

## Invariants

1. No current frontend provider-preference payload contains `*_api_key` values.
2. No current Tauri provider-preference command accepts or returns full provider secrets.
3. `credential_set` rejects blank values and writes keychain only.
4. `credential_delete` deletes keychain only, then recomputes the effective source.
5. Effective credential status is exactly one of `keychain`, `environment`, or `none` and exposes only a masked hint.
6. Environment credentials cannot be deleted from RepoDesk UI; users may create a keychain override.
7. Existing legacy plaintext migration remains fail-safe: plaintext is cleared only after a successful keychain write.
8. Credential mutations invalidate credential status, model health, and the API-environment diagnostic without globally invalidating unrelated queries.
9. `save_codex_quota_status` is non-secret and cannot accidentally return resolved credentials over IPC.
10. Architecture Ratchet and Playwright encode ownership so the duplicate path cannot be casually reintroduced.

## Task 1 — RED: lock the ownership boundary in Architecture Ratchet

**Modify:** `scripts/check-source-architecture.test.mjs`

Add a focused test such as `Credentials have one mutation owner` that reads the canonical frontend/API/Tauri files and initially fails against the current implementation.

Assertions should enforce semantics rather than count labels globally:

- `apps/desktop/src/shared/api/routing.ts` must not invoke `save_provider_settings` or expose API-key fields in the canonical provider preference type.
- `apps/desktop/src/features/settings/useSettings.ts` must not map credential drafts into provider settings or own `saveApiKeys`.
- `apps/desktop/src-tauri/src/lib.rs` must not register `commands::save_provider_settings` and must register a non-secret provider-preference save command.
- `apps/desktop/src/shared/api/credentials.ts` remains the frontend owner of `credential_set` and `credential_delete`.

Do not enforce fragile UI-copy counts here; Playwright covers presentation.

**RED command:**

```bash
node --test scripts/check-source-architecture.test.mjs
```

Expected: the new credential-ownership test fails on the legacy secret-bearing path while existing architecture tests remain green.

Commit the failing guardrail separately so the RED state is explicit.

## Task 2 — RED/GREEN: source-aware effective credential metadata in core

**Modify:** `crates/repodesk-core/src/credentials.rs`

### Tests first

Add resolver-based tests that require:

- keychain value wins over environment value;
- environment is reported when keychain is absent;
- `none` is reported when both are absent/blank;
- returned metadata contains only a masked hint, never the full secret;
- a keychain resolver error is surfaced by status resolution instead of silently becoming `none`;
- environment resolver remains read-only.

Use in-memory/test resolvers; do not touch the real OS keychain in unit tests.

**RED command:**

```bash
cargo test -p repodesk-core credentials
```

### Implementation

Add a stable serialized source enum and effective metadata DTO, for example:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CredentialSource {
    Keychain,
    Environment,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveCredentialMetadata {
    pub key: String,
    pub configured: bool,
    pub hint: String,
    pub source: CredentialSource,
}
```

Add a helper that receives a keychain resolver and environment resolver, reads keychain first, then environment only when keychain is absent/blank, and returns masked effective metadata.

Keep `resolve_secret` runtime semantics unchanged in this task; provenance/status is a separate read contract.

Re-run the focused core test until green.

## Task 3 — RED/GREEN: make desktop credential commands source-aware and single-purpose

**Modify:** `apps/desktop/src-tauri/src/commands/credentials.rs`

### Tests first

Add command-helper tests with in-memory resolvers for:

- blank `credential_set` is rejected and does not delete an existing keychain value;
- successful set returns `source=keychain`;
- deleting keychain override with an environment fallback returns `configured=true`, `source=environment`;
- deleting without fallback returns `source=none`;
- status returns all allowed keys without secret values.

Prefer small internal helper functions parameterized over resolvers, with Tauri wrappers constructing `KeyringResolver`/`EnvResolver`.

**RED command:**

```bash
cargo test -p repodesk-desktop credentials
```

If the desktop package name differs, use the package name declared by `apps/desktop/src-tauri/Cargo.toml`; full workspace tests remain the final authority.

### Implementation

- Validate key against `ALLOWED_KEYS` exactly as today.
- Validate `secret.trim()` is non-empty before calling keychain storage.
- `credential_set` writes only keychain and then computes effective metadata.
- `credential_delete` deletes only keychain and then computes effective metadata, revealing env fallback when present.
- `credential_status` uses source-aware effective metadata rather than reconstructing source-less metadata from `resolve_secret`.
- Do not include raw secrets in error messages.

## Task 4 — RED/GREEN: introduce a non-secret provider-preference store boundary

**Modify:**

- `apps/desktop/src-tauri/src/store/types.rs`
- `apps/desktop/src-tauri/src/store/io.rs`
- relevant existing store tests in `apps/desktop/src-tauri/src/store/`

### Tests first

Add tests proving that saving provider preferences changes only non-secret configuration and does not set, replace, or delete credential-bearing fields/keychain state.

Reuse existing legacy migration tests to preserve these invariants:

- successful keychain migration clears legacy plaintext;
- failed keychain migration keeps the plaintext source of truth.

**Focused RED command:**

```bash
cargo test -p repodesk-desktop store
```

### Implementation

Add `ProviderPreferences` containing every current non-secret provider/routing field but no `anthropic_api_key`, `openai_api_key`, or `gemini_api_key` fields.

Provide explicit conversions/application methods:

- `From<&ProviderSettings> for ProviderPreferences`;
- an apply/update method that copies only preference fields into internal `ProviderSettings` or directly updates the config document.

Add:

```rust
pub fn read_provider_preferences() -> RepoDeskResult<ProviderPreferences>
pub fn save_provider_preferences(preferences: ProviderPreferences) -> RepoDeskResult<ProviderPreferences>
```

`save_provider_preferences` must never call the secret-persistence helper and must preserve existing legacy/keychain credential state untouched.

The internal legacy `ProviderSettings` and `save_provider_settings` may remain temporarily only for bounded compatibility/migration/internal runtime use. They must no longer be reachable as the canonical user preference IPC path.

## Task 5 — RED/GREEN: replace secret-bearing Settings IPC and close the quota leak

**Modify:**

- `apps/desktop/src-tauri/src/commands/settings.rs`
- `apps/desktop/src-tauri/src/lib.rs`

### Tests first

Add command tests proving:

- provider preference read/write returns a non-secret shape;
- preference validation still applies;
- `save_codex_quota_status` uses the non-secret preference path and returns no API-key fields;
- the canonical Tauri invoke surface no longer registers `save_provider_settings`.

### Implementation

Replace the canonical commands with:

```rust
provider_preferences
save_provider_preferences
```

Both accept/return `ProviderPreferences` only.

Refactor `save_codex_quota_status` to:

1. read non-secret preferences;
2. validate/update quota status;
3. save via `save_provider_preferences`;
4. return `ProviderPreferences`.

Do not pass quota updates through `save_provider_settings`.

Remove `commands::save_provider_settings` from `tauri::generate_handler!`; remove `commands::provider_settings` too once frontend consumers are migrated in Task 6.

**Focused command:**

```bash
cargo test -p repodesk-desktop settings
```

## Task 6 — RED/GREEN: migrate shared frontend API and query ownership

**Modify:**

- `apps/desktop/src/shared/api/routing.ts`
- `apps/desktop/src/shared/api/credentials.ts`
- `apps/desktop/src/shared/api/queries.ts`
- any TypeScript consumers discovered by `tsc`

### Implementation contract

In `routing.ts`:

- replace secret-bearing `ProviderSettings` with non-secret `ProviderPreferences`;
- expose `providerPreferences()` -> `invoke("provider_preferences")`;
- expose `saveProviderPreferences()` -> `invoke("save_provider_preferences")`;
- make `saveCodexQuotaStatus()` return `ProviderPreferences`;
- update imports/call sites rather than keeping secret-bearing compatibility aliases.

In `credentials.ts`:

```ts
type CredentialSource = "keychain" | "environment" | "none";
```

and include `source` in credential status metadata.

In `queries.ts`:

- add `queryKeys.credentials.status = ["credential_status"]`;
- change the routing settings key to reflect `provider_preferences` rather than deprecated `provider_settings`.

**Verification:**

```bash
pnpm --dir apps/desktop run build
node --test scripts/check-source-architecture.test.mjs
```

Use TypeScript failures to find remaining old API consumers; do not preserve the deprecated frontend type merely to make compilation easier.

## Task 7 — RED/GREEN: make Settings expose exactly one credential editor

**Modify:**

- `apps/desktop/src/features/settings/SettingsTab.tsx`
- `apps/desktop/src/features/settings/useSettings.ts`
- optionally extract a small `CredentialsSection.tsx` only if it materially reduces Settings complexity without creating a new abstraction layer for one use site

### Remove legacy ownership

Delete from generic provider preferences:

- API-key password fields;
- `keyDraft` ownership;
- `saveApiKeys`;
- `Save API keys` action;
- mutation code that injects `*_api_key` into provider settings.

### Canonical credential states

For each provider credential:

- `keychain`: show `Keychain · <masked hint>`, allow Replace/Save and Delete;
- `environment`: show `Environment · <masked hint>`, no Delete, concise read-only fallback explanation, allow Save override to keychain;
- `none`: show `Not configured`, allow Save to keychain.

Plaintext draft stays component-local and is cleared after successful `credential_set`.

After `credential_set` or `credential_delete`, invalidate/refetch exactly:

- `queryKeys.credentials.status`;
- `queryKeys.models.health`;
- `queryKeys.routing.apiEnv`.

Do not optimistically manufacture credential source state on failed mutations.

## Task 8 — RED/GREEN: Playwright ownership and provenance contract

**Create:** `apps/desktop/e2e/settings-credentials.spec.ts`

**Modify:**

- `apps/desktop/e2e/current-fixtures.ts`
- any older settings-specific fixture still using `provider_settings`, including `apps/desktop/e2e/ide-preferences-ui.spec.ts`

Use `installMockIpc`, `recordedCommands`, and `recordedInvocations` from `apps/desktop/e2e/mock-ipc.ts`.

### Test cases

1. Settings has the one Credentials surface and no legacy `Save API keys` button.
2. `source=environment` renders `Environment`, exposes no Delete action for that provider, and permits a keychain override save.
3. Saving an override invokes `credential_set` with the correct key/secret, clears the input, and causes credential/model-health/API-env refetches.
4. A `credential_status` mock sequence proves delete can transition `keychain` -> `environment` without displaying `Not configured`.
5. Keychain source exposes Delete and invokes `credential_delete` for the correct key.

The mock fixture should use `provider_preferences`, never `provider_settings`, for current Settings.

**RED then GREEN command:**

```bash
pnpm --dir apps/desktop exec playwright test e2e/settings-credentials.spec.ts
```

Then run the full frontend suite:

```bash
pnpm --dir apps/desktop e2e
```

## Task 9 — GREEN: restore Architecture Ratchet and full compile/test surface

Run the focused ownership guardrail again:

```bash
node --test scripts/check-source-architecture.test.mjs
node scripts/check-source-architecture.mjs
```

Then run the same core gates as CI:

```bash
pnpm --dir apps/desktop install --frozen-lockfile
pnpm --dir apps/desktop run build
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
./scripts/secret-scan-basic.sh
pnpm --dir apps/desktop e2e
```

If any failure is non-obvious, stop patching symptoms and use the systematic-debugging workflow to identify the first broken invariant.

## Task 10 — PR, exact-head verification, review, squash merge

Create a draft PR from `refactor/credential-single-owner` to `main` once the RED guardrail is committed so CI history shows the contract becoming green.

Before marking ready:

- fetch the exact PR head SHA;
- require Architecture Ratchet green on that SHA;
- require all CI jobs green on that SHA, including fmt/clippy/tests/frontend, Playwright, cargo-deny, gitleaks, and coverage report;
- require native Tauri/WebDriverIO E2E green on that exact SHA;
- inspect any failure artifacts instead of weakening product assertions to satisfy tests.

Request/review the final diff for:

- secret exposure through serialized types/errors/logging;
- stale credential/model-health cache behavior;
- environment/keychain provenance correctness;
- backward migration safety;
- accidental remaining `save_provider_settings` frontend/IPC call paths.

When exact-head evidence is green, mark PR ready and squash merge into `main`, then verify the merge SHA is the new main head.

## Expected final architecture

```text
Settings Credentials UI
        |
        +---- credential_status --------> effective metadata
        |                                  keychain -> env -> none
        |
        +---- credential_set -----------> OS keychain only
        |
        +---- credential_delete --------> OS keychain only

Settings Provider Preferences UI
        |
        +---- provider_preferences ------> non-secret ProviderPreferences
        +---- save_provider_preferences -> non-secret store update only

Legacy ProviderSettings
        |
        +---- compatibility/runtime/migration only
        +---- NOT a current user-triggered preference/secret IPC contract
```

This plan deliberately keeps complete removal of secret fields from the internal legacy `ProviderSettings` model as a later cleanup unless it becomes trivial while implementing the boundary. The security requirement for this slice is stronger and narrower: no current user action outside dedicated credential commands can mutate or receive provider secrets.