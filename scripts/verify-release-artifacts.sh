#!/usr/bin/env bash
set -euo pipefail

# Verify a tagged RepoDesk release attaches every platform's installer (the N4
# "smoke each / all platform artifacts attached" gate), and — when updater signing
# was active — that latest.json describes all four platforms. Turns a silent,
# partial cross-platform release into a loud failure.
#
# Usage:
#   scripts/verify-release-artifacts.sh <tag>      # queries the GitHub release via gh
#
# Testable without GitHub by injecting inputs:
#   REPODESK_RELEASE_ASSETS=$'a.dmg\nb.deb'  scripts/verify-release-artifacts.sh
#   REPODESK_LATEST_JSON=/path/to/latest.json ...   # to check manifest completeness
#
# Required installers are matched by extension + arch (case-insensitive) so the
# check is robust to productName casing / hyphenation differences across bundlers.

fail=0
note() { printf '  %s\n' "$*"; }

# --- gather asset names ------------------------------------------------------
if [[ -n "${REPODESK_RELEASE_ASSETS:-}" ]]; then
  assets="$REPODESK_RELEASE_ASSETS"
elif [[ $# -ge 1 ]]; then
  command -v gh >/dev/null || { echo "ERROR: gh CLI required to query release '$1'." >&2; exit 2; }
  assets="$(gh release view "$1" --json assets --jq '.assets[].name')"
else
  echo "ERROR: pass a release <tag>, or set REPODESK_RELEASE_ASSETS." >&2
  exit 2
fi

echo "== Verifying release artifacts =="
echo "Assets found:"
printf '%s\n' "$assets" | sed 's/^/  - /'

# --- required installers (label -> case-insensitive ERE) ---------------------
# Keep these in lockstep with bundle.targets in tauri.conf.json.
check_one() {
  local label="$1" regex="$2"
  if printf '%s\n' "$assets" | grep -iEq "$regex"; then
    note "OK   $label"
  else
    note "MISS $label  (no asset matched /$regex/i)"
    fail=1
  fi
}

check_one "macOS dmg (Apple Silicon)" '(aarch64|arm64).*\.dmg$'
check_one "macOS dmg (Intel)"         '(x64|x86_64|amd64|intel).*\.dmg$'
check_one "Linux AppImage"            '\.AppImage$'
check_one "Linux deb"                 '\.deb$'
check_one "Windows installer"         '\.(msi|exe)$'

# --- updater manifest completeness (only when signing was active) ------------
# Signing on => .sig assets exist => latest.json must cover all four platforms.
signed=0
if printf '%s\n' "$assets" | grep -iEq '\.sig$'; then
  signed=1
fi

if [[ "$signed" -eq 1 || -n "${REPODESK_LATEST_JSON:-}" ]]; then
  echo "== Updater manifest (signing active) =="
  if ! printf '%s\n' "$assets" | grep -iEq '(^|/)latest\.json$' && [[ -z "${REPODESK_LATEST_JSON:-}" ]]; then
    note "MISS latest.json  (signing active but no updater manifest attached)"
    fail=1
  else
    manifest="${REPODESK_LATEST_JSON:-}"
    if [[ -z "$manifest" && $# -ge 1 ]]; then
      tmp="$(mktemp -d)"
      gh release download "$1" --pattern latest.json --dir "$tmp" >/dev/null
      manifest="$tmp/latest.json"
    fi
    if [[ -n "$manifest" && -f "$manifest" ]]; then
      for key in darwin-aarch64 darwin-x86_64 linux-x86_64 windows-x86_64; do
        if grep -q "\"$key\"" "$manifest"; then
          note "OK   manifest covers $key"
        else
          note "MISS manifest missing platform '$key'"
          fail=1
        fi
      done
    fi
  fi
else
  echo "== Updater manifest: skipped (no .sig assets — signing not configured) =="
fi

echo
if [[ "$fail" -ne 0 ]]; then
  echo "FAIL: release is missing required artifacts." >&2
  exit 1
fi
echo "OK: all required platform artifacts present."
