# RepoDesk

[![CI](https://github.com/MykytaStel/repodesk/actions/workflows/ci.yml/badge.svg)](https://github.com/MykytaStel/repodesk/actions/workflows/ci.yml)

**Local-first engineering workspace for controlled software change.**

RepoDesk connects a repository and a concrete work item to bounded context, human/agent execution, isolated worktrees, changesets, verification, review, project knowledge, and engineering intelligence.

> RepoDesk is not primarily an AI provider dashboard. AI is one class of worker inside the engineering workflow. The long-term direction is an IDE-like environment centered on `Work Item -> ChangeSet -> Verification -> Knowledge`.

## Current workflow

The current product already implements the core development lifecycle:

```text
Scope -> Prepare -> Execute -> Review -> Verify -> Finish
```

RepoDesk can coordinate local/cloud completion providers and coding-agent executors, build bounded context, run guarded checks, isolate coding-agent writes in Git worktrees, capture diffs, accept/reject changesets, verify reviewed changes, and keep run evidence.

## RepoDesk 2.0 direction

The next product direction expands RepoDesk into an engineering environment with five primary surfaces:

- **Work** — active work item, scope, plan, context, approvals and next action;
- **Code** — repository tree, editor, search, symbols and diagnostics;
- **Changes** — Git state, worktrees, changesets, diff review and commit readiness;
- **Runs** — worker execution, checks, receipts, failures and engineering telemetry;
- **Projects** — repository rules, engineering knowledge, commands and playbooks.

Engineering Intelligence will measure more than AI spend. Planned evidence-backed dimensions include context compactness, worker/agent fan-out, redundant execution, retries and correction cost, knowledge reuse, scope adherence, verification efficiency, and heuristic algorithmic complexity changes.

See:

- [`docs/REPODESK_2_PRODUCT.md`](docs/REPODESK_2_PRODUCT.md) — product contract;
- [`docs/REPODESK_2_MIGRATION.md`](docs/REPODESK_2_MIGRATION.md) — incremental migration roadmap;
- [`docs/ENGINEERING_INTELLIGENCE.md`](docs/ENGINEERING_INTELLIGENCE.md) — telemetry and algorithmic-intelligence model;
- [`docs/architecture/ADR-0001-repodesk-2-product-boundary.md`](docs/architecture/ADR-0001-repodesk-2-product-boundary.md) — RepoDesk/SubRadar boundary.

## Architecture

- `crates/repodesk-core/` — deterministic workflow, context, safety, routing, checks, persistence, worktrees, review, knowledge and orchestration logic;
- `crates/repodesk-cli/` — CLI over the core;
- `apps/desktop/` — Tauri 2 + React/TypeScript desktop application;
- `docs/` — product, architecture, security and roadmap documents;
- `scripts/` — verification, smoke and health scripts.

AI/agent development context lives in [`AGENTS.md`](AGENTS.md).

## Run locally

For safe development without touching the real RepoDesk home:

```bash
export REPODESK_HOME=/tmp/repodesk-dev
cargo run -p repodesk-cli -- init
cargo run -p repodesk-cli -- project add repopilot ~/Documents/projects/repopilot --type rust-cli
cargo run -p repodesk-cli -- project use repopilot
cargo run -p repodesk-cli -- project info
```

Desktop:

```bash
./scripts/dev-desktop.sh
```

## Verify

```bash
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
pnpm --dir apps/desktop build
./scripts/verify-all.sh
```

CI additionally gates secrets and supply-chain checks. See [`AGENTS.md`](AGENTS.md) and [`docs/SECURITY_MODEL.md`](docs/SECURITY_MODEL.md) for the current operational and security contracts.
