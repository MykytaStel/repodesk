# Release Readiness — Your Action Items

This is the list of things **only you** can do (decisions, paid accounts, secrets, repo
settings). Everything code/CI-side from the "N3.5 release hardening" pass is already done
on the branch. Items are ordered: **P0 blocks a public release**, P1 is strongly
recommended, P2 is follow-up.

Legend: ⛔ blocks release · ⚠️ strongly recommended · 💡 nice-to-have

---

## P0 — before distributing to anyone

### ⛔ 1. Choose and add a LICENSE
Right now the repo has **no license**, which legally means "all rights reserved" — others
can't use it. This is a business decision:
- **Open source?** Pick MIT (simplest, permissive) or Apache-2.0 (permissive + patent
  grant). Add a `LICENSE` file and set `license = "MIT"` (or `"Apache-2.0"`) in each
  `crates/*/Cargo.toml` + `apps/desktop/src-tauri/Cargo.toml`.
- **Proprietary/commercial?** Add a custom `LICENSE`/EULA and keep crates `publish = false`
  (already set).
- Then update `deny.toml` is **not** needed (it skips our own crates).
```bash
# Example for MIT (replace NAME/YEAR):
curl -s https://raw.githubusercontent.com/licenses/license-templates/master/templates/mit.txt -o LICENSE
```

### ⛔ 2. Turn on the updater (finish N3)
The auto-updater is wired but won't produce update artifacts until its signing secrets
exist. The keypair was generated locally at `~/.repodesk-keys/` (the **private** key is NOT
in git). Add the two secrets:
```bash
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.repodesk-keys/updater.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD < ~/.repodesk-keys/updater.password
```
**Then back up `~/.repodesk-keys/` somewhere safe (password manager / encrypted storage).**
If you lose it, existing installs can never be updated again.

### ⛔ 3. macOS signing + notarization (Apple Developer ID)
Unsigned builds make every macOS user hit a Gatekeeper "cannot be opened" wall — the #1
adoption blocker. Requires a paid **Apple Developer** account ($99/yr).
1. Create a "Developer ID Application" certificate; export it as `.p12`.
2. Create an app-specific password at appleid.apple.com → Sign-In & Security.
3. Add the secrets (see `docs/RELEASE_CHECKLIST.md` §10):
```bash
base64 -i DeveloperID.p12 | gh secret set APPLE_CERTIFICATE
gh secret set APPLE_CERTIFICATE_PASSWORD     # the .p12 export password
gh secret set APPLE_SIGNING_IDENTITY         # "Developer ID Application: NAME (TEAMID)"
gh secret set APPLE_ID                        # your Apple ID email
gh secret set APPLE_PASSWORD                  # the app-specific password
gh secret set APPLE_TEAM_ID                   # 10-char Team ID
```
The release workflow already consumes these — no code change needed.

### ⛔ 4. Enable GitHub private vulnerability reporting
`SECURITY.md` points reporters to it. Turn it on:
**Repo → Settings → Code security → Private vulnerability reporting → Enable.**

---

## P1 — strongly recommended before a wide launch

### ⚠️ 5. Windows code signing
Without it, Windows users get a SmartScreen warning. Options: an OV/EV Authenticode
certificate (cost + identity verification) or Azure Trusted Signing. This isn't wired yet —
tell me and I'll add the `tauri-action` Windows signing inputs once you have a cert.

### ⚠️ 6. Run an updater canary end-to-end
The updater flow has never run against a real signed release (I can't test it without your
signing secret). After step 2:
1. Bump `tauri.conf.json` version to e.g. `1.0.1`, tag `v1.0.1`, let the release build.
2. Install `1.0.0`, then confirm it detects + applies `1.0.1`.
This is the real proof N3 works. (I can add an automated canary workflow if you want.)

### ⚠️ 7. Legal/privacy review
`PRIVACY.md` describes actual behavior but is **not legal advice**. If you distribute
commercially (especially in the EU), have someone confirm your privacy/data-processing
obligations — the app can send code to third-party AI APIs when a cloud provider is enabled.

### ⚠️ 8. Require the new CI checks (branch protection)
After this branch merges and CI goes green once, require the checks on `main`:
```bash
# Required check names: the CI job names, e.g.
#   "Gates (fmt, clippy, tests, frontend, secret-scan)"
#   "E2E smoke (Playwright, mock IPC)"
#   "Supply chain (cargo-deny)"
#   "Secret scan (gitleaks)"
# Set via: Repo → Settings → Branches → Add branch protection rule (or `gh api`).
```

---

## P2 — follow-ups / maturity

- 💡 **9. Pin GitHub Actions to commit SHAs** (supply-chain hardening) — currently `@v0/@v4`.
- 💡 **10. Align crate versions** — `crates/*` are `0.1.0` while the app is `1.0.0`. The
  version guard only checks `tauri.conf.json`; align if you want them to match.
- 💡 **11. SBOM** — generate a CycloneDX SBOM per release (enterprise buyers increasingly
  ask). I can add `cargo-cyclonedx` to the release workflow on request.
- 💡 **12. gitleaks license** — if you move this repo into a GitHub **org**, the gitleaks
  Action needs a free `GITLEAKS_LICENSE` secret (personal repos don't).
- 💡 **13. Telemetry stance** — decide whether you want opt-in anonymous usage metrics
  (currently none — fully local-first). If yes, it must be explicit opt-in to preserve trust.

---

## What's already done for you (this branch)
- `SECURITY.md`, `PRIVACY.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, issue templates.
- Supply-chain gate (`deny.toml` + `cargo-deny` CI job) — vulnerabilities, licenses, sources.
- Secret scanning via `gitleaks` (CI) on top of the existing basic scan.
- Coverage report job (non-gating).
- Tag↔version release guard (`scripts/check-version-sync.sh` + `version-check` job).
- Hardened "context never leaks file bodies" test (now uses a real git repo).
