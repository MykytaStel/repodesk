# RepoDesk Orchestrator

The orchestrator turns the **active task** into a plan of **sub-agent tasks**, routes each
to the cheapest capable model, and runs them in dependency order — each with its **own
bounded, Memory-Brain-injected context pack**. This is RepoDesk's move from a human-in-the-loop
*coach* to a *conductor* that actually runs agents.

## Why

1. **Context-window preservation.** The orchestrator's own context stays lean — it holds only
   the plan and each sub-agent's final result, never their intermediate reasoning. Each
   sub-agent reasons inside its own pack.
2. **Cost via per-task model selection.** Each step is routed through the existing
   [`routing`](../crates/repodesk-core/src/routing) engine, so cheap/local models (Ollama) do
   the bulk work and premium models (Claude/OpenAI) are used only where a step needs them.
3. **Coordination through the shared Memory Brain.** A finished sub-agent's output is passed
   directly to its dependents *in-run*, and is also captured as **human-reviewable Memory Brain
   proposals**. The durable brain still requires propose→accept; only the in-run handoff is
   automatic.

## Architecture

```
active task ──▶ plan (analyze → implement → review)        crates/repodesk-core/src/orchestrator/plan.rs
                  │  each step routed via routing::route_request (per-task provider+model)
                  ▼
              runner (topological order)                   .../orchestrator/runner.rs
                  │  per step: own context pack ──────────  .../orchestrator/context.rs (reuses smart_context + memory slice)
                  │            safety scan · budget verdict · cost ceiling
                  │            provider.complete(...) ─────  crates/repodesk-core/src/api_clients/{anthropic,openai,gemini,ollama}.rs
                  │            log token ledger · capture → Memory Brain proposals
                  ▼
              OrchestrationRun (persisted to <run_dir>/orchestrate/<run_id>.json + latest.json)
```

### Provider clients
`LlmProvider::complete(LlmRequest) -> LlmResponse` ([api_clients/mod.rs](../crates/repodesk-core/src/api_clients/mod.rs))
adds model selection, an optional system prompt, an output token cap, a `ThinkingLevel`, and
real token usage. `provider_for(name, &ProviderSettings)` builds the right client:
`ollama`/local → Ollama; `anthropic`/`claude` → Anthropic; `chatgpt`/`codex`/`gpt`/`openai` →
OpenAI; `gemini` → Gemini. Keys come from the environment:
`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY` (or `GOOGLE_API_KEY`); optional
`*_BASE_URL` / `*_MODEL` overrides. Ollama needs no key.

### Routing & availability
Planning only routes to providers it can actually call: `available_capacities` drops paid
providers without a configured key, so with no keys everything routes to local Ollama (and
write/patch steps that local models may not do fall back to a clearly-flagged **manual** step
at zero cost).

### Gates (every step, before any spend)
- **Safety** — `safety::scan_text` blocks if secret-like content is in the outgoing context.
- **Budget** — `evaluate_context` blocks contexts above the configured token block limit.
- **Cost ceiling** — `--max-cost` halts before a step that would exceed the budget.
- Paid runs require explicit confirmation (`--yes` in the CLI; a confirm dialog in the UI).

## CLI

```bash
repodesk orchestrate plan [--goal "..."]                 # route every step; no execution
repodesk orchestrate run --dry-run [--max-cost N]        # gate + project cost, no provider calls
repodesk orchestrate run [--goal "..."] [--max-cost N] --yes
repodesk orchestrate status                              # most recent run for the active task
repodesk orchestrate show <run_id>
```

Offline preview (no network, no keys):

```bash
export REPODESK_HOME=/tmp/repodesk-dev
repodesk init
repodesk project add myproj /path/to/repo --type rust-cli
repodesk project use myproj
repodesk task new "My task"
repodesk orchestrate plan
repodesk orchestrate run --dry-run
```

With a key configured (e.g. `OPENAI_API_KEY`), the write/patch step routes to a paid provider
while analyze/review stay on local Ollama — the cost-savings mix.

## Desktop

The **Orchestrate** tab previews the plan (provider/model/thinking/deps per step), runs it
(dry-run or real, with an optional cost ceiling and a paid-run confirmation), shows per-step
tokens/cost/status and run totals, and deep-links into the **Memory** tab to review captured
proposals. Tauri commands: `orchestrate_plan`, `orchestrate_run`, `orchestrate_status`,
`orchestrate_show`.

## Outcome ledger (N8-A — the "Hermes" learning signal)

Every **real** (non-dry-run) step is recorded to a `run_outcomes` SQLite table
(migration v3) by `outcomes::record_run` after the run is persisted: the routed
provider/model, token usage, cost, and a **verdict** (`good` for a clean step,
`bad` for a failure/block, `neutral` for a skipped/manual step). The verdict
starts provisional (`verdict_source = auto`) and only becomes authoritative when
a human confirms it (`outcomes confirm <id> <good|bad>` → `verdict_source =
human`) — the same propose→approve discipline the Memory Brain uses.

`outcomes::outcome_stats(project)` aggregates the ledger into per-(task_kind,
provider) success rates and average cost. This is **read-only fuel** for the
adaptive router (N8-B); recording an outcome never changes routing on its own.

```bash
repodesk outcomes list [--limit N]      # recent step outcomes, newest first
repodesk outcomes stats                 # success rate + avg cost per kind/provider
repodesk outcomes confirm <id> <good|bad|neutral>
```

## Invariants

- Durable Memory Brain mutations stay human-approved (propose→accept); only in-run handoff is automatic.
- Local-first & bounded: per-sub-agent packs are token-budgeted; the full repo is never ingested.
- Never send secrets; honor each agent's write permission.

## Deferred

- **N8-B adaptive routing**: feed `outcome_stats` back into `routing::scoring` as a learned
  bias, so the router prefers what has actually worked on *this* project (kept explainable, not
  a black box; confirmed `human` verdicts weighted above provisional `auto` ones).
- **N8-C autonomous loop + LLM planning**: a "run task to completion" mode (plan → run → checks
  → re-plan/retry on failure under budget/safety guardrails) and LLM-assisted decomposition to
  replace the static analyze→implement→review template.
