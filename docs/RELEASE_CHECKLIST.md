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
- [ ] macOS: sign with a Developer ID and notarize (`APPLE_*` / signing identity).
      Unsigned builds run locally but Gatekeeper will warn other users.
- [ ] Windows: Authenticode sign the installer.
- [ ] Linux: builds are unsigned by convention.

## 6. Auto-updater (currently DISABLED)
- [ ] The updater was removed in P2 (demo pubkey, no signing key). Before enabling:
      - Generate a real key pair (`tauri signer generate`).
      - Add the `tauri-plugin-updater` dependency.
      - Restore the `plugins.updater` block with the **real** pubkey + a trusted
        endpoint, and widen CSP `connect-src` to that endpoint only.
      - Sign release artifacts and publish the updater manifest.

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
