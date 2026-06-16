# AGENTS.md — RepoDesk

Shared context for any AI/agent (Claude Code, Codex, Cursor, …) working in this repo.
Read this first; it exists so you don't have to re-derive the project each session.

## What this is
RepoDesk is a **local-first desktop "AI operations cockpit"** for AI-assisted development:
connect a project → scope one task → build bounded context → run safety/budget checks →
route work to the right (preferably local) model → know when it's safe to commit.

## Layout
- `crates/repodesk-core/` — deterministic core logic (workflow engine, routing, guard/safety/
  security, checks, persistence/SQLite, memory, orchestrator). Most logic lives here.
- `crates/repodesk-cli/` — `repodesk` CLI over the core.
- `apps/desktop/` — Tauri 2 app: React/TS frontend in `src/`, Rust backend in `src-tauri/`.
  Tauri commands are in `src-tauri/src/commands/`; the workflow engine is fed in `commands/workflow.rs`.
- `docs/` — ROADMAP, SECURITY_MODEL, RELEASE_CHECKLIST, POST_V1_PLAN, NEXT_DEVELOPMENT_PLAN.
- `scripts/` — verify/smoke/health scripts.

## Build / test / verify (run these; keep them green)
- Tests: `cargo test --workspace`. **Use `REPODESK_HOME=/tmp/repodesk-dev` for any CLI or
  stateful test** so the real `~/.repodesk` is never touched.
- Format check: `cargo fmt --all -- --check`.
- Lint: `cargo clippy --all-targets --all-features -- -D warnings`.
- Frontend: `npm --prefix apps/desktop run build` (tsc + vite).
- Scripts: `./scripts/verify-fast.sh` (fast gate), `./scripts/verify-all.sh` (full gate),
  `./scripts/secret-scan-basic.sh`, `./scripts/health-report.sh`, `./scripts/smoke-desktop.sh`.
- Run the app: `./scripts/dev-desktop.sh`. Build a bundle: `npm --prefix apps/desktop run desktop:build`.

## Conventions (do these)
- Keep source files focused/small. Core logic is deterministic and **pure-function +
  unit-tested**; new behavior needs a test.
- Stateful core tests use a temp `REPODESK_HOME` fixture and `#[serial]` (see
  `crates/repodesk-core/tests/core_safety_paths.rs`) because env + global stdio are process-wide.
- Respect the **check-command allowlist** security model — no shell metacharacters; only
  whitelisted binaries (`crates/repodesk-core/src/checks.rs`).
- Local-first: paid/cloud agents are disabled unless explicitly allowed and never receive secrets.
- Branch per change; the repo's history is feature-branch → merge into `main`.
  **Do not push or commit unless the human asks.**

## Gotchas (save yourself time)
- **rustfmt repo-wide drift**: `cargo fmt --all -- --check` can flag many pre-existing files.
  Format only the files you touched: `rustfmt --edition 2024 <leaf-file>`.
- **Build/test the workspace**, not a single crate in isolation where avoidable; crate feature
  unification matters (tokio `rt-multi-thread` lives in core's Cargo.toml).
- The desktop `run_cli` shim uses **process-global stdout capture** — do not assert on its
  captured content in parallel tests (it races libtest). It is DEPRECATED; prefer calling core directly.
- `build_context` never dumps raw repo file contents (only RepoDesk-managed files + git
  metadata) — keep it that way. Secret scanning gates content before AI use.

## CI/CD
- `.github/workflows/ci.yml` — gates (fmt, clippy, tests, frontend build, secret-scan) on
  every PR + push to `main`. Mirror it locally with `./scripts/verify-all.sh`.
- `.github/workflows/release.yml` — push a tag `vX.Y.Z` to build all-platform installers via
  `tauri-action` and open a **draft** GitHub Release (keep `tauri.conf.json` version in sync).
- `.github/workflows/audit.yml` — weekly RustSec advisory scan (informational).
- `.github/dependabot.yml` — weekly grouped dependency PRs (cargo / npm / actions).

## Security model
See `docs/SECURITY_MODEL.md` (threat model + enforcement map). Summary: bounded context by
construction, path denylist + traversal guard on file reads, safety/judge gate before AI,
confirm-before-paid-AI at hand-off, value-focused pre-commit secret scan, tight Tauri CSP +
minimal capability, auto-updater disabled until real signing key.

## Status & roadmap
- **v1.0 reached** — MVP→Product phases P1–P7 done; an (unsigned) `RepoDesk_1.0.0_aarch64.dmg`
  builds and validates locally.
- **Next**: `docs/POST_V1_PLAN.md` (N1 CI → N2 E2E → N3 signing/updater → N4 cross-platform →
  N5 remove `run_cli` debt → N6 product depth). General direction in `docs/ROADMAP.md`.
</content>
