# RepoDesk: Post-v1.0 Development Plan

v1.0 reached: the MVP→Product phases (P1–P7) are complete and an (unsigned) aarch64 macOS
bundle (`RepoDesk_1.0.0_aarch64.dmg`) builds and validates locally. This plan takes RepoDesk
from "builds on my machine" to "ships reliably to others", then grows product depth.

## Known gaps blocking a reliably-shipped product
1. **No CI** — gates run manually on the dev machine.
2. **No live/GUI verification** — logic is unit-tested; the app has never been driven
   end-to-end automatically.
3. **Release pipeline incomplete** — only an unsigned aarch64 macOS bundle; no notarization,
   no signed installers, auto-updater disabled.
4. **Known debt** — `run_cli` does in-process CLI dispatch with global stdout redirection
   (DEPRECATED; was the flaky-test source).

## Phases

### N1 — CI + automated gates (highest leverage)
GitHub Actions runs `fmt`, `clippy -D warnings`, `cargo test --workspace`,
`secret-scan-basic.sh`, and the frontend build on every PR + `main`. Cache cargo + node.
**Exit:** red PRs are blocked; `verify-all.sh` is mirrored in CI.

### N2 — Live + E2E verification  ✅ (hybrid)
`tauri-driver` + WebdriverIO (or Playwright) smoke that launches the app and completes the
daily loop (onboard → context → checks → commit-readiness), asserting key UI states; a
first-run test against a throwaway `REPODESK_HOME`. **Exit:** one command runs an automated
GUI smoke, wired into CI where headless is possible.

**Done — hybrid, because `tauri-driver` can't run on macOS (WKWebView has no WebDriver):**
- **Playwright + mock IPC** (`apps/desktop/e2e/`, `./scripts/e2e-smoke.sh`): drives the real
  React daily-loop UI in headless Chromium with a faked Tauri IPC layer (`mock-ipc.ts` defines
  `window.__TAURI_INTERNALS__`). Runs anywhere incl. macOS; gates every PR via `ci.yml`.
  Covers onboarded daily loop + first-run onboarding; asserts the frontend issues the loop's
  commands through IPC. No app changes needed.
- **tauri-driver + WebdriverIO** (`apps/desktop/e2e-native/`, `./scripts/e2e-native.sh`):
  real-backend smoke against the compiled binary + `repodesk-core`, first-run against a
  throwaway `REPODESK_HOME`. Linux only → runs in `e2e-native.yml` on push to `main`/dispatch
  (heavy full build; not yet a per-PR gate).

### N3 — Signing, notarization, auto-updater  ◑ (updater done; signing wired, secrets pending)
macOS Developer ID signing + notarization in CI (secrets-gated). Re-enable
`tauri-plugin-updater` with a real key + trusted endpoint (see `RELEASE_CHECKLIST.md` §6) and
narrow CSP `connect-src` to it. **Exit:** signed/notarized artifacts on tagged releases.

**Done:**
- **Auto-updater re-enabled** with a real minisign key (public key in `tauri.conf.json`;
  private key generated locally, kept out of the repo, to be stored as a CI secret). Endpoint =
  GitHub Releases `latest.json`; `connect-src` narrowed to `github.com` +
  `objects.githubusercontent.com`; `updater:default` capability granted; plugin registered in
  `lib.rs`. Installs only signature-verified bundles, checked explicitly (not on launch).
  `release.yml` produces `.sig` + `latest.json` (`includeUpdaterJson`, `createUpdaterArtifacts`).
- **Signing/notarization plumbing** wired into `release.yml` (`TAURI_SIGNING_*` + `APPLE_*`),
  dormant until secrets exist. Setup steps in `RELEASE_CHECKLIST.md` §10.

**Pending (needs your inputs):** add the updater signing secrets to make a tagged release
actually ship update artifacts; obtain an Apple Developer ID + add the `APPLE_*` secrets to
reach the "signed/notarized artifacts" exit.

### N4 — Cross-platform release
CI matrix builds `.dmg` (x86_64 + aarch64), `.AppImage`/`.deb`, `.msi`; smoke each.
**Exit:** a tagged release attaches all platform artifacts.

### N5 — Replace deprecated `run_cli` dispatch
Call `repodesk-core` services directly from Tauri commands (data is already structured);
delete `run_cli`, the `stdio_override` usage, and the allowlist shim. **Exit:** no
process-global stdio redirection in the desktop crate; actions return typed results.

### N6 — Product depth (pick by value)
RepoPilot inline per-file findings in the Code tab + auto-run review + trend over time;
multi-task / multi-project switching with per-task history; finish the multi-agent
orchestrator; broader provider support (LM Studio, richer model health, cost trends).

## Suggested order
N1 → N2 → N3 → N4 is the "ship reliably" backbone (in order). N5 is debt cleanup that makes
N2 less flaky (slot near N2). N6 is growth, after the release pipeline is trustworthy.

## Working agreement
Branch per phase; keep all gates green; new behavior needs a test; use
`REPODESK_HOME=/tmp/repodesk-dev` for CLI testing; report what changed + commands + results;
don't push or commit without the human's say-so.
</content>
