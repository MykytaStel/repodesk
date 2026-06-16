# RepoDesk Release Checklist

RepoDesk ships as a local-first Tauri desktop app. This checklist takes a green
`main` to a packaged, installable build.

## 1. Pre-flight (must be green)
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace` (run a few times — no flaky failures)
- [ ] `./scripts/verify-all.sh`
- [ ] `./scripts/secret-scan-basic.sh` exits 0
- [ ] `npm --prefix apps/desktop run build` (tsc + vite) clean

## 2. Version + metadata
- [ ] Bump `version` in `apps/desktop/src-tauri/tauri.conf.json`.
- [ ] Keep `productName`, `identifier` (`dev.repodesk.desktop`), and bundle
      `category`/descriptions/`copyright` accurate.
- [ ] (Optional) align `apps/desktop/src-tauri/Cargo.toml` and workspace crate
      versions with the app version.
- [ ] Update `CHANGELOG`/release notes.

## 3. Icons
- [ ] Source icon is ≥ 1024×1024.
- [ ] Regenerate the icon set: `npm --prefix apps/desktop run tauri -- icon <source.png>`
      (produces `icons/icon.icns`, `icon.ico`, sized PNGs, store logos).
- [ ] Confirm `bundle.icon` in `tauri.conf.json` references the generated files.

## 4. Build the bundle
- **Automated (preferred):** push a tag `vX.Y.Z` → `.github/workflows/release.yml`
  (tauri-action) builds macOS (arm64 + x64), Linux, and Windows installers and opens a
  **draft** GitHub Release with the artifacts. Review the draft, then publish.
- **Local (manual):** `npm --prefix apps/desktop run desktop:build` (alias for `tauri build`);
  artifacts land under `target/release/bundle/` (macOS `.app`/`.dmg`, Linux `.AppImage`/`.deb`,
  Windows `.msi`/`.exe`).

## 5. Signing & notarization (per platform)
The release workflow (`.github/workflows/release.yml`) already wires the signing
env into `tauri-action`; it stays **dormant** (unsigned build, no error) until the
secrets below exist, then signs + notarizes automatically.
- [ ] macOS: add the Developer ID signing + notarization secrets (see §10). Unsigned
      builds run locally but Gatekeeper warns other users.
- [ ] Windows: Authenticode sign the installer (not yet wired).
- [ ] Linux: builds are unsigned by convention.

## 6. Auto-updater (ENABLED — GitHub Releases endpoint)
The updater is re-enabled (N3): `tauri-plugin-updater` is registered in
`src-tauri/src/lib.rs`, the `updater:default` capability is granted, and
`tauri.conf.json` carries the **real** public key plus the endpoint
`https://github.com/MykytaStel/repodesk/releases/latest/download/latest.json`.
CSP `connect-src` is narrowed to `github.com` + `objects.githubusercontent.com`.
The plugin only verifies/installs **signed** bundles and is **not** triggered on
launch (local-first: update checks are explicit, not background).
- [ ] Updater signing secrets (`TAURI_SIGNING_PRIVATE_KEY[_PASSWORD]`, see §10) are
      set — without them, a tagged build produces installers but **no** `.sig` /
      `latest.json`, so updates won't resolve.
- [ ] After a release publishes, confirm `latest.json` is attached to the GitHub Release.
- [ ] Rotating the key: `tauri signer generate`, replace `plugins.updater.pubkey`,
      update both secrets. Old installs can only update to builds signed by the key
      whose pubkey they shipped with.

## 10. Required GitHub Actions secrets
Set these in the repo (Settings → Secrets and variables → Actions, or `gh secret set`).

**Updater (enables auto-update artifacts):**
- `TAURI_SIGNING_PRIVATE_KEY` — minisign private key produced by `tauri signer generate`.
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — its password.

```bash
# From a checkout, using the locally generated key (kept outside the repo):
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.repodesk-keys/updater.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD < ~/.repodesk-keys/updater.password
```

**macOS Developer ID signing + notarization (dormant until all set):**
- `APPLE_CERTIFICATE` — base64 of the exported Developer ID `.p12`
  (`base64 -i cert.p12 | pbcopy`).
- `APPLE_CERTIFICATE_PASSWORD` — the `.p12` export password.
- `APPLE_SIGNING_IDENTITY` — e.g. `Developer ID Application: Your Name (TEAMID)`.
- `APPLE_ID` — your Apple ID email.
- `APPLE_PASSWORD` — an app-specific password (appleid.apple.com → Sign-In & Security).
- `APPLE_TEAM_ID` — your 10-char Team ID.

## 7. Local data safety
- [ ] Confirm Backup/Restore works (Debug tab → Local data): back up, then restore.
- [ ] First run on a clean machine: onboarding (connect project → create task) works.

## 8. Smoke the packaged build
- [ ] Launch the packaged app on a clean machine/profile.
- [ ] Run the daily loop end-to-end: connect project → task → build context →
      checks → RepoPilot review → commit-readiness → guarded commit.
- [ ] Verify no unrestricted shell, no paid AI call without confirm (see
      `docs/SECURITY_MODEL.md`).

## 9. Tag & publish
- [ ] Tag the release commit (`vX.Y.Z`).
- [ ] Attach platform artifacts to the release.
- [ ] Publish release notes.
</content>
