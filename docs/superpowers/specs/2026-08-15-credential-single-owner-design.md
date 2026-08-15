# Credential Single-Owner Architecture

Date: 2026-08-15
Status: Design approved; implementation pending written-spec review

## Context

RepoDesk already has the correct storage direction in `repodesk-core`: packaged builds use the OS keychain, environment variables are a read-only development fallback, and the desktop store safely migrates legacy plaintext provider keys toward the keychain.

The remaining defect is ownership convergence. Settings currently exposes two ways to manage the same API keys:

1. legacy `Bring your own keys` password fields saved through generic provider settings; and
2. the dedicated keychain credential editor saved through `credential_set` / `credential_delete`.

The backend mirrors that ambiguity because generic provider-settings persistence can still mutate credentials. `credential_status` also reports the effective keychain-or-environment value without reporting the source, so an environment fallback can be rendered as `Stored` and offered a misleading `Delete` action.

For a security-critical concept, RepoDesk must have one visible owner and one mutation boundary.

## Goals

This slice makes the dedicated credential path the only user-triggered owner of provider secrets while preserving safe migration and development compatibility.

The completed implementation must guarantee:

- one visible API-credential editor in Settings;
- only dedicated credential commands can perform user-triggered secret writes;
- OS keychain is canonical writable storage;
- environment variables remain read-only fallback;
- effective credential provenance is explicit but non-secret;
- ordinary provider-preference saves cannot set, replace, or delete credentials;
- deleting a keychain override reveals an existing environment fallback;
- credential mutations refresh dependent health/diagnostic state;
- legacy plaintext migration remains fail-safe;
- no full secret crosses credential-status IPC or enters frontend persistence, logs, debug previews, or run artifacts.

## Non-goals

This slice does not:

- remove environment-variable fallback;
- add providers;
- redesign provider/model routing;
- add credential import/export;
- make native E2E depend on a real OS keychain in headless CI;
- perform a broad provider-settings schema rewrite unrelated to securing the single-writer boundary.

A later cleanup may remove secret-shaped fields from the internal legacy `ProviderSettings` model after the user-facing and command-level ownership boundary is stable.

## Ownership model

### Credential mutations

There are exactly two canonical user-triggered credential operations:

- `credential_set`: set/replace a keychain credential;
- `credential_delete`: delete a keychain credential.

`credential_set` rejects blank input. Deletion is owned only by `credential_delete`; blank-as-delete is removed from the canonical command behavior so the API has one explicit deletion semantic.

Generic provider preference mutations cannot carry API-key fields and cannot call credential persistence internally.

### Provider preferences

Introduce a non-secret `ProviderPreferences` payload containing only provider/model/economy configuration required by Settings. The current Settings UI saves preferences through `save_provider_preferences`.

`save_provider_preferences` updates only non-secret provider fields. It never accepts or derives API-key values.

The old Tauri `save_provider_settings` command is removed from the current invoke surface in this slice. The lower-level legacy store read/migration machinery may remain internally while runtime consumers still depend on it, but it is not a user-triggered secret writer.

### Storage versus resolution

Writable credential storage is the OS keychain. Effective runtime resolution is:

1. usable keychain value;
2. usable environment variable;
3. no credential.

Environment credentials are never persisted implicitly and cannot be deleted by RepoDesk.

## Core credential contract

Add source-aware effective metadata in `repodesk-core`:

```rust
#[serde(rename_all = "lowercase")]
pub enum CredentialSource {
    Keychain,
    Environment,
    None,
}

pub struct CredentialMetadata {
    pub key: String,
    pub configured: bool,
    pub hint: String,
    pub source: CredentialSource,
}
```

Stable serialized source values are:

- `keychain`
- `environment`
- `none`

The credential layer provides a helper that computes effective metadata without returning a full secret to the desktop UI.

### Resolution semantics

For a supported key:

- usable keychain value -> `configured=true`, `source=keychain`, masked keychain hint;
- otherwise usable environment value -> `configured=true`, `source=environment`, masked environment hint;
- otherwise -> `configured=false`, `source=none`, empty hint.

A real keychain access failure is not silently treated as key absence when doing so could hide a storage failure. Status commands surface a bounded error instead of lying that the credential is unconfigured or falling through to environment on an unreadable keychain.

Trusted runtime code may still resolve full credentials internally. Full secret values are never part of credential-status metadata.

## Desktop IPC contract

### `credential_status`

Returns source-aware metadata for all supported provider credentials.

It never labels a resolved value merely as `Stored`; the caller receives the effective source explicitly.

### `credential_set`

- accepts an allowed credential key and a non-blank value;
- writes only to the OS keychain;
- rejects blank/whitespace-only values;
- returns recomputed effective metadata;
- never echoes the submitted secret.

After a successful write the effective source is `keychain`.

### `credential_delete`

- deletes only the keychain entry;
- recomputes effective metadata after deletion;
- if an environment fallback exists, returns `configured=true`, `source=environment`;
- never mutates the process environment.

### `save_provider_preferences`

Accepts only the non-secret `ProviderPreferences` shape and updates only non-secret provider settings. API-key fields are absent by type and command contract.

This is the only generic provider-settings write used by the current Settings UI.

## Legacy migration contract

Preserve the existing fail-safe migration behavior in the desktop store:

1. read legacy plaintext credential if present;
2. if no usable keychain value exists, attempt to store the legacy credential in the keychain;
3. clear the legacy plaintext database value only after keychain persistence succeeds;
4. if keychain persistence fails, preserve the legacy value so the only copy is not destroyed;
5. environment fallback is read-only and is never copied to persistent storage implicitly.

Saving ordinary provider preferences must not trigger credential set/delete operations and must not rewrite legacy credential columns.

## Frontend design

Settings contains two distinct concepts:

1. non-secret provider/model defaults and preferences;
2. `Credentials`, the sole API-key management surface.

Remove the legacy password inputs and `Save API keys` action from generic provider settings.

### Credential card states

#### Keychain

Display:

`Keychain · ••••1234`

Actions:

- enter a replacement value and save to keychain;
- delete the keychain credential.

#### Environment

Display:

`Environment · ••••1234`

Actions:

- no `Delete` action for the environment value;
- allow entering a value to create a keychain override;
- explain concisely that the environment credential is read-only from RepoDesk.

#### None

Display:

`Not configured`

Action:

- enter and save a value to the keychain.

Plaintext input exists only as ephemeral component state. It is cleared after a successful mutation. The server never returns the full value into React Query.

### Failure state

A credential-status failure is rendered as an explicit bounded error state. It must not degrade to `Not configured`, because that would misrepresent security state.

A failed set/delete does not optimistically change source or hint.

## Query ownership and invalidation

Add a canonical query key:

```ts
queryKeys.credentials.status
```

After successful `credential_set` or `credential_delete`, invalidate/refetch:

- `queryKeys.credentials.status`;
- `queryKeys.models.health`;
- `queryKeys.routing.apiEnv`.

Do not use global `invalidateQueries()` for this mutation. Do not invalidate unrelated routing snapshots unless implementation inspection proves they depend on credential state.

## Runtime consumers

`model_health_snapshot` currently obtains effective credentials through `read_provider_settings`. That store path already resolves keychain first, safely migrates legacy values, and supports environment fallback. Therefore it is functionally compatible with this slice.

This slice secures the user-triggered write boundary first. Runtime consumers are moved directly to `CredentialResolver` only if the implementation is a small mechanical change with no routing/model-health behavior expansion. Otherwise complete removal of secret fields from internal `ProviderSettings` remains a separate follow-up.

This separation prevents a security ownership fix from turning into an unnecessary platform-wide model rewrite.

## Error handling and security invariants

- Keychain set/delete/read errors surface without secret material.
- A failed mutation is never reported as success.
- Environment fallback is never mutated by RepoDesk.
- Legacy plaintext is never cleared before successful keychain migration.
- Masked hints are non-reversible display metadata only.
- Secret values are not persisted in frontend storage.
- Secret-bearing command arguments remain redacted by existing debug-preview protections.
- Generic preference APIs contain no secret fields, so future UI changes cannot accidentally reintroduce a second secret writer through that path.

## Verification strategy

### Architecture Ratchet

Add ownership regression coverage that proves:

- generic Settings/provider-preference code contains no API-key password editor and does not invoke legacy secret-saving commands;
- current frontend credential code owns `credential_set` / `credential_delete`;
- `ProviderPreferences` and `save_provider_preferences` are non-secret by shape;
- current Tauri invoke registration does not expose `save_provider_settings` as the normal provider write path.

The test enforces mutation ownership, not a brittle repository-wide duplicate-string count.

### Core/unit tests

Use resolver abstractions/in-memory fakes instead of requiring a desktop keychain in unit CI. Cover:

- keychain source wins when present;
- environment source is used only when keychain is absent;
- none when neither exists;
- keychain read failure is surfaced rather than misreported;
- masked metadata never exposes the full credential;
- environment resolver remains read-only.

### Command/store tests

Cover:

- blank `credential_set` is rejected;
- deleting a keychain override recomputes to environment fallback when present;
- saving `ProviderPreferences` does not set, delete, or replace credentials;
- successful legacy migration clears plaintext only after keychain storage succeeds;
- failed keychain migration preserves the legacy plaintext value.

Reuse existing migration coverage when it already proves an invariant.

### Playwright

Using mock IPC, prove:

1. Settings exposes one credential editor and no legacy `Save API keys` surface;
2. `source=environment` renders explicitly and has no environment-delete action;
3. saving invokes `credential_set`, clears plaintext input, and refreshes credential/health state;
4. deleting a keychain credential invokes `credential_delete` and can transition the card to environment fallback metadata;
5. generic preference saving never sends API-key fields.

### Native E2E

The existing native Tauri/WebDriverIO suite must remain green. Do not add a dependency on a real OS keychain service in headless native CI for this UI contract.

## Acceptance criteria

The slice is complete only when all are true:

1. Settings has exactly one API-key management surface.
2. Canonical frontend secret writes use only `credential_set` / `credential_delete`.
3. `credential_set` rejects blank values; `credential_delete` owns deletion.
4. Generic provider preference payloads/commands contain no credential fields and cannot mutate credentials.
5. Credential status reports `keychain`, `environment`, or `none` without returning a full secret.
6. Deleting a keychain override correctly reveals an environment fallback.
7. Model Health and API/environment diagnostics refresh after credential mutation.
8. Existing fail-safe legacy plaintext migration remains intact and tested.
9. No new plaintext secret persistence, logging, debug exposure, or frontend persistent storage is introduced.
10. Architecture Ratchet and Playwright enforce the ownership contract.
11. Full Rust/frontend CI, cargo-deny, gitleaks, coverage, Architecture Ratchet, and native Tauri E2E are green on the exact final PR head before squash merge.

## Follow-up boundaries

After this slice, remove credential fields from the remaining internal legacy `ProviderSettings` model and move runtime consumers directly onto credential resolution if that cleanup still provides value. Keep it separate unless implementation proves mechanically trivial.

Repository branch protection remains an independent P0 operational trust gap and should be fixed separately rather than mixed into credential code.