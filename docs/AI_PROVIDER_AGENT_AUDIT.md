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
| CLI auth detection | `authenticated: Option<bool>` carried but left `None` (no documented status command) | ⚠️ partial (honest unknown) |
| Routing → executor selection | patch steps route to PATH-available agent, else manual; paid floor enforced | ✅ implemented |
| Orchestrator execution | `ExecutorKind::CodingAgent` arm runs the CLI behind `approve_coding_agents` | ✅ implemented |
| Cost/token ledger for agents | output captured → `estimate_agent_cost` + `log_token_event` | ✅ implemented |
| Paid/agent confirmation | `approve_paid` + `approve_coding_agents` are separate gates (CLI `--yes`, desktop switches) | ✅ implemented |
| Desktop agent status panel | "Executor availability" panel lists binary/path/status/version | ✅ implemented |
| Changed-file/diff in agent result | `CodingAgentExecution` captures pre/post porcelain delta + unified diff + receipt path; surfaced on `SubAgentResult` and the run panel | ✅ implemented |
| Worktree lifecycle | `git_workspace.rs` is read-only snapshot/diff; no *isolated* worktree per run (diff is captured in-place against the project tree) | ❌ absent |
| Secure credential store | env vars only (`OPENAI_API_KEY` etc.); no OS keychain | ❌ absent |
| OpenAI Responses API | client still uses Chat Completions transport | ❌ not migrated |
| Review / accept-reject flow | `orchestrator::review` stages (accept) or discards (reject) a run's changeset; CLI + Tauri + run-panel buttons | ✅ implemented |

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
   panel shows the changed-file count + list. Diff is captured **in-place** against
   the project tree (no isolated worktree yet); untracked new files are listed but
   their content is not inlined.
9. **Accept / reject review** — `orchestrator::review` turns a captured changeset
   into an action: **accept** stages the agent-changed files (`git add`), **reject**
   discards them (`git restore --source=HEAD` for tracked, `git clean` for
   untracked). Bounded to the run's recorded paths, path-validated, never commits
   or pushes. Exposed via `repodesk orchestrate review`, the `orchestrate_review`
   Tauri command, and run-panel buttons.

## Remaining gaps (ordered)

1. **CLI auth detection** — turn `authenticated: None` into a real tri-state once
   a documented, side-effect-free status command exists per CLI (e.g. a `whoami`
   / `status` subcommand). Do **not** parse credential files.
2. **Git worktree lifecycle** — run write-capable agents inside an isolated
   worktree (instead of the in-place diff capture done today), so the diff is
   attributable even on a dirty tree; then accept (apply) or reject (discard) —
   with enough metadata to recover an interrupted run. Keep lifecycle code out of
   the provider clients.
4. **Secure credential store** — OS keychain abstraction (macOS Keychain, Windows
   Credential Manager, Linux Secret Service) behind a `CredentialResolver`; SQLite
   keeps only non-secret metadata (configured flag, default model, masked hint).
   Never return a full key to the frontend.
5. **OpenAI Responses API migration** — move `openai.rs` off Chat Completions,
   preserving usage extraction, rate-limit handling, model selection, and tests.
6. **Review depth** — the accept/reject core exists; remaining polish is an
   inline diff viewer for the captured `diff_path` and an optional cross-model
   review of the changeset before the accept/reject decision.
7. **Local-runtime ownership decision** — keep Ollama/LM Studio as probe-only, or
   let RepoDesk manage their process lifecycle. Current behavior is probe-only.
8. **Deprecation warnings** — user-visible warnings for legacy route aliases
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
