#!/usr/bin/env bash
set -euo pipefail

echo "== RepoDesk basic secret scan =="

if ! command -v git >/dev/null 2>&1; then
  echo "WARN: git is not installed; skipping git grep based scan"
  exit 0
fi

PATTERN='(api[_-]?key|secret|password|passwd|private[_-]?key|authorization:|bearer[[:space:]]+[a-zA-Z0-9._-]+|token[[:space:]]*=|access[_-]?token|refresh[_-]?token)'

set +e
RESULTS=$(git grep -n -I -E "$PATTERN" -- \
  ':!target' \
  ':!tmp' \
  ':!apps/desktop/node_modules' \
  ':!apps/desktop/dist' \
  ':!*.lock' \
  ':!docs/SECURITY_MODEL.md' \
  ':!scripts/secret-scan-basic.sh' 2>/dev/null)
STATUS=$?
set -e

if [[ $STATUS -eq 0 && -n "$RESULTS" ]]; then
  echo "WARN: potential sensitive strings found:"
  echo "$RESULTS"
  echo "Review these before committing. False positives are possible."
  exit 1
fi

echo "OK: no obvious sensitive strings found"
