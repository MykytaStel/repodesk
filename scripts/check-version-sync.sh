#!/usr/bin/env bash
set -euo pipefail

# Guard: the release tag must match the app version in tauri.conf.json. A mismatch
# silently breaks the updater (latest.json advertises a version users never see as an
# upgrade) — so we fail the release before anything is built.
#
# Usage:
#   scripts/check-version-sync.sh v1.2.3      # compares against the given tag
#   scripts/check-version-sync.sh             # just prints the configured version
#
# Accepts tags with or without a leading 'v' (v1.2.3 or 1.2.3).

cd "$(dirname "$0")/.."

conf="apps/desktop/src-tauri/tauri.conf.json"
conf_version="$(jq -re '.version' "$conf")"
echo "tauri.conf.json version: $conf_version"

tag="${1:-}"
if [[ -z "$tag" ]]; then
  echo "No tag passed; nothing to compare."
  exit 0
fi

tag_version="${tag#v}"   # strip optional leading v
if [[ "$tag_version" != "$conf_version" ]]; then
  echo "ERROR: release tag '$tag' (version '$tag_version') does not match" >&2
  echo "       $conf .version = '$conf_version'." >&2
  echo "       Bump tauri.conf.json (and re-tag) so they agree before releasing." >&2
  exit 1
fi

echo "OK: tag '$tag' matches configured version '$conf_version'."
