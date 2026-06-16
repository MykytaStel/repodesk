# Changelog

All notable changes to RepoDesk are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **CI/CD (N1):** GitHub Actions gates (fmt, clippy, tests, frontend build, secret-scan)
  on every PR and push to `main`.
- **E2E (N2):** Playwright daily-loop smoke with a mocked Tauri IPC layer (runs everywhere,
  gates PRs) plus a real-backend `tauri-driver` + WebdriverIO smoke (Linux/CI).
- **Auto-updater (N3):** re-enabled with a real signing key and a GitHub Releases endpoint;
  CSP `connect-src` narrowed to the updater hosts; macOS/updater signing wired into the
  release workflow (dormant until secrets are added).
- **Cross-platform release verification (N4):** `verify-release` job asserts every platform
  installer (and a complete `latest.json` when signed) is attached to a tagged release.
- **Release hardening (N3.5):** `SECURITY.md`, `PRIVACY.md`, `CONTRIBUTING.md`, issue/PR
  templates; a tag↔version sync guard; `cargo-deny` supply-chain gate; gitleaks secret
  scanning; a coverage report job.

### Changed
- Frontend tooling standardized on **pnpm**.

## [1.0.0] - 2026

Initial v1.0: local-first AI operations cockpit. MVP→Product phases P1–P7 complete —
workflow engine, routing, safety/security/guard gates, checks allowlist, SQLite
persistence, memory, orchestrator, and a packaged (unsigned) macOS bundle.

[Unreleased]: https://github.com/MykytaStel/repodesk/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/MykytaStel/repodesk/releases/tag/v1.0.0
