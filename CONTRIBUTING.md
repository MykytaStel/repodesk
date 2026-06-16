# Contributing to RepoDesk

Thanks for your interest! This guide covers the essentials. For the full project
context (layout, conventions, gotchas), read [`AGENTS.md`](AGENTS.md) first.

## Prerequisites
- Rust (stable; `rustfmt` + `clippy`).
- Node 20 + **pnpm** (the frontend uses pnpm, not npm).
- Tauri 2 system deps for desktop builds (see `.github/workflows/ci.yml` for the Linux list).

## Build / test / verify
Always use a throwaway home for stateful runs so your real `~/.repodesk` is untouched:
```bash
REPODESK_HOME=/tmp/repodesk-dev cargo test --workspace
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
pnpm --dir apps/desktop install
pnpm --dir apps/desktop run build
```
Convenience gates:
- `./scripts/verify-fast.sh` — quick gate during iteration.
- `./scripts/verify-all.sh` — full gate (mirror of CI) before handoff.
- `./scripts/e2e-smoke.sh` — Playwright daily-loop smoke (mocked IPC; runs anywhere).

## Conventions
- Keep source files focused and small. Core logic is deterministic, pure-function, and
  unit-tested — **new behavior needs a test** (see `crates/repodesk-core/tests/`).
- Stateful core tests use a temp `REPODESK_HOME` fixture and `#[serial]`.
- Respect the check-command allowlist (`crates/repodesk-core/src/checks.rs`) — no shell
  metacharacters, only whitelisted binaries.
- Local-first: paid/cloud agents stay disabled unless explicitly enabled, and never
  receive secrets.
- Format only the files you touched: `rustfmt --edition 2024 <file>`.

## Pull requests
- Branch per change (`feat/…`, `fix/…`); the history is feature-branch → merge into `main`.
- Keep all gates green; CI runs them on every PR.
- Describe the change, the commands you ran, and the results (see the PR template).
- Don't commit secrets. `./scripts/secret-scan-basic.sh` and gitleaks run in CI.

## Security
Report vulnerabilities privately — see [`SECURITY.md`](SECURITY.md). Do not open public
issues for security problems.
