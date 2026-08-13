# Dependency Risk Register

RepoDesk upgrades or removes actionable vulnerable dependencies instead of globally suppressing RustSec findings. Exceptions are narrow, documented here, and must have an explicit removal condition.

## Accepted upstream platform risk

### RUSTSEC-2024-0429 — `glib` < 0.20 unsound `VariantStrIter`

- **Status:** temporarily accepted transitive platform risk.
- **Current path:** the Linux Tauri/WebKitGTK runtime pulls the GTK3 / `glib` 0.18 dependency line.
- **RepoDesk direct use:** RepoDesk does not directly call `glib::VariantStrIter` or the affected iterator APIs.
- **Impact:** the affected API is unsound and can produce undefined behaviour/crashes when exercised.
- **Control:** `.cargo/audit.toml` ignores only this advisory while continuing to deny other unsound findings and security vulnerabilities.
- **Removal condition:** remove the exception immediately when the supported Tauri Linux runtime no longer requires `glib` < 0.20 (for example after an upstream GTK4/newer gtk-rs migration), then verify with `cargo audit` and `cargo deny check`.
- **Review cadence:** every Rust/Tauri dependency update and at least weekly through the security-audit workflow.

## Unmaintained GTK3 ecosystem

RustSec also marks the GTK3 `gtk-rs` bindings (for example `gtk`, `gtk-sys`, `gdk`, `atk`, and related 0.18 packages) as unmaintained. These are transitive Linux desktop dependencies of the current WebKitGTK/Tauri stack, not RepoDesk-owned libraries. `cargo-deny` therefore treats unmaintained third-party transitives as warnings rather than merge blockers.

These warnings must not be copied into the `ignore` list individually. The exit condition is the same platform migration described above; until then they remain visible in scheduled audit output.

## Remediated in the August 2026 audit closure

The compatible lockfile refresh removes the actionable versions that triggered the current audit findings:

- `anyhow` 1.0.102 → 1.0.104 (RUSTSEC-2026-0190 patched in >= 1.0.103)
- `event-listener` 5.4.1 → 5.4.2
- `quick-xml` 0.39.4 → 0.41.0 (RUSTSEC-2026-0194 and RUSTSEC-2026-0195 patched in >= 0.41.0)

No exception is permitted for those remediated advisories.
