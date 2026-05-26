#!/usr/bin/env bash
set -euo pipefail

mkdir -p tmp
OUT="tmp/repodesk-health-report.md"

{
  echo "# RepoDesk Health Report"
  echo
  echo "Generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo

  echo "## Git"
  echo '```txt'
  git branch --show-current 2>/dev/null || true
  git status --short 2>/dev/null || true
  git log -1 --oneline 2>/dev/null || true
  echo '```'
  echo

  echo "## Tool versions"
  echo '```txt'
  rustc --version 2>/dev/null || true
  cargo --version 2>/dev/null || true
  node --version 2>/dev/null || true
  npm --version 2>/dev/null || true
  echo '```'
  echo

  echo "## Cargo check"
  echo '```txt'
  cargo check --workspace 2>&1 || true
  echo '```'
  echo

  if [[ -f "apps/desktop/package.json" ]]; then
    echo "## Desktop build"
    echo '```txt'
    npm --prefix apps/desktop run build 2>&1 || true
    echo '```'
    echo
  fi

  echo "## Secret scan"
  echo '```txt'
  ./scripts/secret-scan-basic.sh 2>&1 || true
  echo '```'
} > "$OUT"

echo "Health report written to $OUT"
