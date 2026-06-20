# AI Providers And Coding Agents Audit

Source brief: `repodesk_ai_providers_agents_prompt_and_estimate.md` (separating
completion providers from coding-agent executors). This doc tracks the
current-state audit, what landed, and the ordered gaps that remain.

## Core principle

A **completion provider** receives a bounded request over HTTP and returns text.
A **coding-agent executor** is a local CLI that can read files, run commands, and
edit a working tree. They must never share an identity: `codex` must not silently
mean "OpenAI API", and `claude` must not silently mean "Anthropic API".

```
RepoDesk
├── Completion providers       api_clients/  (openai_api, anthropic_api, gemini_api, ollama, lm_studio)
├── Coding-agent executors     executors.rs  (codex_cli, claude_code_cli)
├── Deterministic check runner checks.rs
├── Manual hand-off            routing ExecutorKind::Manual
└── Router + orchestrator + safety + usage ledger
```

## Current-state table

| Concern | Implementation | Status |
|---|---|---|
| Executor vs provider identity | `ExecutorKind` + `executor_id` + `provider_id` on routes (`routing/types.rs`) | ✅ implemented |
| Canonical provider ids | `openai_api`, `anthropic_api`, `gemini_api`, `ollama`, `lm_studio` in `provider_for` | ✅ implemented |
| Canonical agent ids | `codex_cli`, `claude_code_cli` (`executors.rs::canonical_coding_agent_id`) | ✅ implemented |
| Ambiguous alias rejection | `provider_for("codex"/"claude")` rejected; they are not completion ids | ✅ implemented |
| Coding-agent execution contract | `CodingAgentSpec`/`CodingAgentCommandSpec`/`CodingAgentExecution` | ✅ implemented |
| Safe process runner | argv-only, no `sh -c`, stdin prompt, timeout+kill, receipt files, output capture | ✅ implemented |
| Read-only vs workspace-write | `build_coding_agent_command(.., writes_allowed)` → codex `--sandbox`, claude `--permission-mode` | ✅ implemented |
| CLI availability (passive) | PATH lookup, no spawn (`coding_agent_availability`) | ✅ implemented |
| CLI version detection | bounded `<binary> --version` probe (`coding_agent_availability_probed`) | ✅ implemented |
| CLI auth detection | `authenticated: Option<bool>` set `Some(true)` from a known auth-artifact *existence* check (never `Some(false)`, never reads contents) | ✅ implemented (conservative) |
| Routing → executor selection | patch steps route to PATH-available agent, else manual; paid floor enforced | ✅ implemented |
| Orchestrator execution | `ExecutorKind::CodingAgent` arm runs the CLI behind `approve_coding_agents` | ✅ implemented |
| Cost/token ledger for agents | output captured → `estimate_agent_cost` + `log_token_event` | ✅ implemented |
| Paid/agent confirmation | `approve_paid` + `approve_coding_agents` are separate gates (CLI `--yes`, desktop switches) | ✅ implemented |
| Desktop agent status panel | "Executor availability" panel lists binary/path/status/version | ✅ implemented |
| Changed-file/diff in agent result | `CodingAgentExecution` captures pre/post porcelain delta + unified diff + receipt path; surfaced on `SubAgentResult` and the run panel | ✅ implemented |
| Worktree lifecycle | `worktree.rs`: create/list/status/remove isolated per-step worktrees; runner requires isolation for approved coding-agent runs | ✅ implemented |
| Secure credential store | `credentials.rs`: `CredentialResolver` + OS keychain (`keyring`) + env fallback; Tauri set/delete/status; never returns the full key | ✅ implemented |
| OpenAI Responses API | `openai.rs` migrated to `/v1/responses` (input/instructions/max_output_tokens/reasoning.effort) | ✅ implemented |
| Review / accept-reject flow | `orchestrator::review` stages/discards in-place changesets and safely applies isolated worktree changes back on accept | ✅ implemented |

## What landed (PRs 1–8 equivalent)

1. **Provider/executor separation** — `ExecutorKind`, `executor_id`, `provider_id`
   on `ProviderCapacity`/`RouteCandidate`/`RouteDecision`; legacy `provider`
   fields kept for UI/API back-compat.
2. **Canonical ids + alias safety** — completion ids and agent ids are disjoint;
   `codex`/`claude` no longer resolve to API clients.
3. **Execution contract + safe runner** — argv-only command specs, stdin prompt
   transport, shell-metacharacter rejection, timeout + kill, stdout/stderr
   receipt files with size-limited captured notes.
4. **Codex CLI + Claude Code executors** — read-only and workspace-write command
   shapes (`codex exec --sandbox …`, `claude --print --permission-mode …`).
5. **Routing integration** — only PATH-available agents are routable; write/patch
   steps fall back to a zero-cost **manual** step when no safe executor exists.
6. **Orchestrator integration** — the `CodingAgent` arm runs the CLI only with
   explicit approval, accounts cost/tokens from captured output, and feeds output
   into Memory Brain proposals like any other step.
7. **CLI version probe** — `coding_agent_availability_probed` runs
   `<binary> --version` (bounded, argv-only) so the desktop shows a real version
   and "present but not runnable" is distinguishable from "available". The
   `authenticated` field exists and is reported as `None` until a safe status
   command is identified per CLI.
8. **Agent-run diff capture** — the executor snapshots `git status --porcelain`
   before/after a run and records `changed_files` (the delta), a size-limited
   unified `diff`, and a `diff_path` receipt on `CodingAgentExecution`. The runner
   propagates `changed_files`/`diff_path` to `SubAgentResult`, and the desktop run
   panel shows the changed-file count + list. Untracked new files are listed but
   their content is not inlined.
9. **Accept / reject review** — `orchestrator::review` turns a captured changeset
   into an action. For in-place legacy runs, **accept** stages the agent-changed
   files (`git add`) and **reject** discards them (`git restore --source=HEAD` for
   tracked, `git clean` for untracked). For isolated worktree runs, **accept**
   applies the worktree diff back only after same-path conflict checks, stages the
   applied result, and copies untracked files only when the destination is absent;
   **reject** leaves the active checkout untouched and keeps the worktree for
   manual inspection/cleanup. Bounded to the run's recorded paths, path-validated,
   never commits or pushes.
10. **CLI auth tri-state** — `coding_agent_availability_probed` sets
    `authenticated = Some(true)` when a known local auth artifact (e.g.
    `~/.codex/auth.json`, `~/.claude/.credentials.json`) *exists*; never reads its
    contents, never treats an API-key env var as CLI auth, and never reports
    `Some(false)` (absence ≠ unauthenticated — keychain/env may hold it).
11. **Git worktree lifecycle** — `worktree.rs` creates/lists/removes isolated
    per-step worktrees checked out at HEAD. Approved coding-agent runs require an
    isolated workspace by default; creation failure blocks the step and no CLI is
    launched. Worktree ids include run + step identity plus collision-resistant
    suffixes, stale paths are never force-removed, and recovery metadata is written
    before launch. Non-destructive: the worktree is left for review and cleanup is
    explicit. Lifecycle code stays out of executor clients.
12. **OS keychain credential store** — `credentials.rs` defines `CredentialResolver`
    with `KeyringResolver` (macOS Keychain / Windows Credential Manager / Linux
    Secret Service via `keyring`) and a read-only `EnvResolver` fallback. Tauri
    `credential_set`/`credential_delete`/`credential_status` store/clear/inspect
    keys; only masked metadata (`••••1234`) ever crosses to the frontend, never the
    secret. Runs consult the keychain when env/settings are unset.
13. **OpenAI Responses API** — `openai.rs` now calls `/v1/responses`
    (`input`/`instructions`/`max_output_tokens`, `reasoning.effort` when thinking is
    set), preserving usage extraction, rate-limit handling, and model selection.
14. **Worktree recovery/cleanup UI** — RepoDesk lists managed isolated worktrees for
    the active task in the desktop Recovery panel and via
    `repodesk orchestrate worktrees`. The status includes run/step metadata, path,
    dirty state, changed-file names, and warnings. Cleanup is explicit via the UI or
    `repodesk orchestrate cleanup-worktree <workspace_id>`, validates the workspace
    id/path against RepoDesk's managed parent and git's worktree registry, then
    removes the worktree plus recovery metadata.

## Remaining gaps (ordered)

1. **CLI auth depth** — upgrade the existence-based `authenticated` signal to a real
   tri-state once a documented, side-effect-free status command exists per CLI.
2. **Credential-store migration** — move the legacy plaintext provider-settings keys
   into the keychain and stop persisting them in app files.
3. **Review depth** — inline diff viewer for the captured `diff_path` and an optional
   cross-model review of the changeset before the accept/reject decision.
4. **Local-runtime ownership decision** — keep Ollama/LM Studio as probe-only, or let
   RepoDesk manage their process lifecycle. Current behavior is probe-only.
5. **Deprecation warnings** — user-visible warnings for legacy route aliases
   (`preferred_patch_provider = "codex"` is normalized to `codex_cli` on read).

## Verification

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
pnpm --dir apps/desktop install
npm --prefix apps/desktop run build
./scripts/verify-fast.sh
./scripts/secret-scan-basic.sh
```

Use `REPODESK_HOME=/tmp/repodesk-dev` for stateful CLI/integration tests.
