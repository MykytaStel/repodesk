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
real token usage. `provider_for(name, &ProviderSettings)` builds completion-provider clients
only:
`ollama`/local → Ollama; `lm_studio` → local OpenAI-compatible endpoint;
`anthropic_api`/`anthropic` → Anthropic; `openai_api`/`openai`/`chatgpt`/`gpt` → OpenAI;
`gemini_api`/`gemini` → Gemini. `codex`, `codex_cli`, `claude`, and `claude_code_cli` are
not completion-provider ids; they belong to the coding-agent executor layer. Keys come from the environment:
`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY` (or `GOOGLE_API_KEY`); optional
`*_BASE_URL` / `*_MODEL` overrides. Ollama needs no key.

### Routing & availability
Planning only routes to providers it can actually call: `available_capacities` drops paid
completion providers without a configured key, and only keeps CLI coding agents when their
binary is found on PATH by passive lookup. With no keys everything routes to local Ollama where
allowed; write/patch steps can route to a PATH-available coding-agent executor, otherwise they
fall back to a clearly-flagged **manual** step at zero cost.

### Coding-agent executors
`executors.rs` defines the CLI-agent boundary for `codex_cli` and `claude_code_cli`.
It normalizes legacy aliases, builds argv-only command previews with the bounded prompt carried
on stdin, and reports availability at two levels:

- **Passive** (`coding_agent_availability`) — a PATH lookup only; no process is spawned. Used by
  routing/planning and the runtime status, where listing must stay cheap and side-effect-free.
- **Probed** (`coding_agent_availability_probed`) — passive lookup *plus* a bounded
  `<binary> --version` probe (argv-only, 5s timeout, no `sh -c`). Running the CLI's own version
  command confirms the binary is actually runnable and surfaces its `version`. `authenticated`
  stays `None` (unknown) on purpose: neither CLI exposes a documented, side-effect-free
  auth-status command, and RepoDesk never parses credential files. The desktop "Executor
  availability" panel uses this variant.

The runner launches these commands only when `RunOptions.approve_coding_agents` is true (CLI
`--yes`, or the desktop's explicit CLI-agent approval); otherwise it records the handoff as
skipped. Executions capture stdout/stderr to receipt files, enforce a timeout, and never fall
back from a coding-agent executor to an OpenAI/Anthropic completion client.

**Changeset capture.** Around each run the executor snapshots `git status --porcelain` before
and after, so a write-capable run produces a reviewable changeset, not just stdout:
`CodingAgentExecution` carries `changed_files` (the porcelain delta), a size-limited unified
`diff` of the tracked changes (staged + unstaged; new untracked files are listed but not
inlined), and a `diff_path` receipt file. The runner surfaces the changed-file list + diff path
on the step result (`SubAgentResult.changed_files` / `diff_path`), and the desktop run panel
shows the count and file list. This is the foundation for the planned accept/reject review flow
— RepoDesk still never commits, pushes, or merges on its own.

### Gates (every step, before any spend)
- **Safety** — `safety::scan_text` blocks if secret-like content is in the outgoing context.
- **Budget** — `evaluate_context` blocks contexts above the configured token block limit.
- **Cost ceiling** — `--max-cost` halts before a step that would exceed the budget.
- Paid providers and coding-agent CLIs require explicit confirmation (`--yes` in the CLI; separate
  approvals in the UI).

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
provider) success rates and average cost (raw, for human display). Recording an
outcome never changes routing on its own.

## Adaptive routing (N8-B — the brain learns)

`outcomes::routing_bias(project)` turns the ledger into a `RouteBias`: for each
(task_kind, provider) pair it sums good/bad verdicts (human-confirmed rows
weighted double), and — once a pair clears a minimum signal weight — emits a
**bounded score adjustment** `(rate − 0.5) · 2 · MAX · confidence`, where
confidence ramps with evidence. `routing::route_request_with_bias` applies this
as a nudge on top of the deterministic score: it can break a tie or sway a close
call toward what has worked on *this* project, but it **never unblocks a route,
touches the Manual/CheckRunner floors, or overrides a hard rule**, and every
applied nudge is recorded as a candidate warning so the decision stays
explainable. `build_plan` feeds the active project's bias into every step; the
bias is empty (a pure no-op) until the ledger has enough signal, so routing stays
deterministic out of the box. `repodesk runtime route --need …` shows the nudge
in its warnings.

```bash
repodesk outcomes list [--limit N]      # recent step outcomes, newest first
repodesk outcomes stats                 # success rate + avg cost per kind/provider
repodesk outcomes confirm <id> <good|bad|neutral>
```

## Autonomous loop (N8-C — the brain drives)

`orchestrator::run_loop(goal, &LoopOptions)` turns a single attempt into
**attempt → evaluate → re-plan → retry** until the task succeeds or a stop
condition fires. It is bounded and explainable by construction:

- **Bounded**: a `max_iterations` cap and an optional `max_total_cost` ceiling
  (the per-attempt cost ceiling is whatever total budget remains).
- **Human-in-the-loop**: with `approve_paid = false` or `approve_coding_agents = false`
  the loop refuses to execute matching gated steps and stops with
  `LoopStatus::NeedsApproval` — no spend or CLI launch without explicit approval.
- **Guardrail-aware**: a safety/budget block stops the loop with
  `GuardrailBlocked` (a retry would hit the same wall); a plain failure retries.
- **Learning is free**: each real attempt records outcomes to the ledger, so the
  next attempt's `build_plan` re-reads the updated routing bias and routes around
  a provider that just failed — no extra machinery in the loop.

Terminal states: `Succeeded`, `NeedsApproval`, `GuardrailBlocked`, `Exhausted`,
`DryRun` (a single preview pass).

```bash
repodesk orchestrate loop [--goal "..."] [--max-iterations N] [--max-cost N] [--dry-run] [--yes]
```

## Invariants

- Durable Memory Brain mutations stay human-approved (propose→accept); only in-run handoff is automatic.
- Local-first & bounded: per-sub-agent packs are token-budgeted; the full repo is never ingested.
- Never send secrets; honor each agent's write permission.

## Deferred

- **LLM-assisted decomposition**: a dynamic, per-attempt plan instead of the static
  analyze→implement→review template. Left out of the bounded loop core because it is
  non-deterministic and needs a live model to verify.
- **Desktop UI** for the outcome ledger, the learned bias, and the autonomous loop (core +
  CLI only so far).
