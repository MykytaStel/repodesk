# RepoDesk Security Model

RepoDesk is local-first and should treat every AI/runtime/provider as a bounded module, not as a trusted operator.

## Core rules

1. The desktop UI must not expose unrestricted shell execution.
2. All desktop actions must go through explicit Rust/Tauri allowlists.
3. Secret files must not be sent to AI context.
4. Paid agents must only receive bounded context packs.
5. Local tools like Ollama are preferred for compression and low-risk review.
6. Patch/execution agents require guard/judge checks before action.
7. Debug output should be visible, but must avoid leaking secrets.

## Blocked or sensitive paths

- .env
- .env.*
- *.pem
- *.key
- credentials.*
- secrets.*
- id_rsa
- id_ed25519
- node_modules/
- target/
- .git/

## Desktop action policy

Allowed actions should be named, explicit, and auditable.

Good:

- build_context
- build_smart_context
- safety_scan
- run_project_checks
- generate_prompts
- git_workspace_snapshot

Bad:

- run_shell_command
- execute_script
- arbitrary_command
- write_file_anywhere
- read_secret_file

## AI provider policy

Local providers:

- Allowed for summaries, compression, low-risk analysis.
- Must remain local-only unless explicitly configured otherwise.

Paid/cloud providers:

- Disabled by default where possible.
- Must show token/cost/security warning before use.
- Should receive smart-context, not full repository dumps.

## Logging policy

Store:

- action id
- command/action name
- timestamp
- duration
- status
- short error/result summary

Avoid storing:

- API keys
- auth headers
- raw secret files
- large raw logs without filtering

## Threat model and enforcement (where the rules actually live)

This section maps the rules above to the code that enforces them, and is honest
about the limits. Updated during P2 (security hardening).

### Assets we protect
- Secrets on the developer's machine (`.env`, keys, credentials, provider API keys).
- The integrity of what leaves the machine toward paid/cloud agents.
- The user's repository (no unrestricted writes/exec from the UI).

### Trust boundaries
- **The active project directory is trusted** by the developer who registered it.
  Project checks run there.
- **Paid/cloud agents are untrusted sinks.** Anything handed to them must be bounded
  and secret-free.
- **The desktop webview is sandboxed.** It can only call allowlisted Rust commands.

### Enforcement map
1. **Check-command allowlist** — `repodesk_core::checks::is_allowed_check_command`.
   Only a fixed set of build/test/lint binaries; rejects shell metacharacters
   (`; & | < > $ \` ( ) \\`, newlines) and any non-allowlisted/binary-by-path command.
   *Limit:* checks run via `sh -c` in the trusted project dir, and tools like
   `cargo`/`npm`/`make` execute project-authored code by design. The allowlist is a
   guard against obviously dangerous binaries and shell chaining — **not** a sandbox.
2. **Context is bounded by construction** — `repodesk_core::context::build_context`
   only ingests RepoDesk-managed files (task md, memory/decisions/risks) plus git
   *metadata* (`branch`, `status --short`, `diff --stat`, `--name-only`). It never
   dumps arbitrary repo file contents. Regression-tested in
   `tests/core_safety_paths.rs::build_context_does_not_leak_repo_file_contents`.
3. **Path denylist + traversal guard** — `repodesk_core::security::is_blocked_path`
   is enforced in the desktop file-read commands (`read_code_file`,
   `code_workbench_snapshot` in `src-tauri/src/lib.rs`): rejects denylisted paths,
   `..`, absolute paths, and any path that escapes the project root (canonicalize +
   `starts_with`).
4. **Safety scan before AI** — `safety::scan_active_context` and
   `security::scan_text_for_secrets` flag secrets/keys; `judge::judge_agent` composes
   guard preflight + safety scan + token budget into ALLOW/WARN/BLOCK.
5. **Paid-agent hand-off gate** — `paid_agent_gate` (Tauri command) re-runs the judge
   pipeline when the user reveals a paid-agent prompt and forces an explicit
   confirmation. RepoDesk never sends prompts automatically; the user copies them out.
6. **Pre-commit secret scan** — `scripts/secret-scan-basic.sh` detects literal secret
   *values* (quoted long literals assigned to secret-named fields, `AKIA…`,
   private-key blocks, `ghp_`/`sk_live_`/`xox…` prefixes), not identifiers.

### Desktop (Tauri) posture
- CSP: `default-src 'self'`, no `unsafe` in `script-src`. `connect-src` is narrowed to
  `'self'` plus exactly the updater endpoint hosts (`github.com`,
  `objects.githubusercontent.com`) — the only outbound webview calls; provider calls go
  through Rust.
- Capability scoped to `core:default` + `updater:default` for the `main` window; no
  `fs`/`http`/`shell` plugins are enabled.
- The auto-updater is **enabled** with a real minisign key (public key in
  `tauri.conf.json`; private key + password are CI secrets only) and a trusted GitHub
  Releases endpoint. The plugin installs **only** signature-verified bundles, and update
  checks are **explicit**, never run automatically on launch (local-first).

### Known limitations (non-goals for now)
- The check allowlist does not sandbox the commands it permits.
- Local provider calls (Ollama) are localhost and not treated as exfiltration.
- The basic secret scan is heuristic; it is a gate, not a guarantee.
