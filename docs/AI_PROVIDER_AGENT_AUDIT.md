# AI Providers And Coding Agents Audit

Source brief: `/Users/mykyta/Downloads/repodesk_ai_providers_agents_prompt_and_estimate.md`.

## Current State

- Completion providers and coding-agent executors were mixed under the same
  `provider` name in several layers.
- `provider_for("codex")` previously built an OpenAI completion client, and
  `provider_for("claude")` previously built an Anthropic completion client.
  That made a future Codex CLI / Claude Code integration unsafe because a route
  label could silently become an API call.
- Routing already had a `PatchAgent` kind, but route decisions exposed only
  `recommended_provider`, so UI/orchestrator code could not distinguish API
  providers from executors.
- The desktop app can enable local endpoints such as Ollama and LM Studio, but
  it does not install or launch those runtimes. The app only probes configured
  endpoints.
- The orchestrator can call completion providers and local LLM endpoints. It
  does not yet execute Codex CLI or Claude Code.

## First PR Scope

- Add explicit route identity:
  - `ExecutorKind`
  - `executor_id`
  - `provider_id`
- Canonical completion-provider ids:
  - `ollama`
  - `lm_studio`
  - `openai_api`
  - `anthropic_api`
  - `gemini_api`
- Canonical coding-agent executor ids:
  - `codex_cli`
  - `claude_code_cli` (reserved for the executor PR)
- Keep legacy `provider` / `recommended_provider` fields for frontend and API
  compatibility.
- Reject ambiguous coding-agent aliases in `provider_for`; `codex` and
  `claude` no longer resolve to completion-provider clients.
- Keep CLI coding-agent execution out of this PR. Coding-agent routes are
  identified cleanly, but the runner does not spawn those tools yet.
- Add a dedicated executor boundary for coding agents:
  - canonical alias normalization (`codex` → `codex_cli`, `claude` →
    `claude_code_cli`)
  - passive PATH availability
  - argv-only command preview
  - stdin prompt transport
  - no `sh -c` and no automatic CLI launch

## Remaining Gaps

- Add real process execution for `codex_cli` and `claude_code_cli` behind the
  executor boundary.
- Add CLI auth detection beyond passive PATH availability.
- Feed CLI availability into routing capacities instead of manual status
  toggles.
- Add an in-app terminal or terminal panel only after the executor boundary and
  command sandbox rules are designed.
- Decide whether RepoDesk should start local runtimes itself or only guide the
  user to run Ollama/LM Studio externally. Current behavior is probe-only.
- Add user-visible deprecation warnings for old route aliases such as
  `preferred_patch_provider = "codex"`. The app now normalizes that value to
  `codex_cli` on read/save.

## Follow-Up PRs

1. Implement the coding-agent executor interface without shell metacharacters or
   `sh -c`, starting with a dry-run/logging mode.
2. Add Codex CLI execution behind guard, budget, safety, and explicit
   confirmation gates.
3. Add Claude Code CLI execution with the same gates.
4. Add a bounded in-app terminal surface if it is needed for user-visible
   executor sessions.
5. Add local-runtime management only if RepoDesk should own process lifecycle
   for Ollama/LM Studio; otherwise keep local model setup as probe + docs.
