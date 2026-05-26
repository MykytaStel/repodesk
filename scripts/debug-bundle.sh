#!/usr/bin/env bash
set -euo pipefail

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
DIR=".repodesk-debug/$STAMP"
mkdir -p "$DIR"

echo "Creating debug bundle in $DIR"

{
  echo "# RepoDesk Debug Bundle"
  echo "Generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} > "$DIR/README.md"

{
  echo "## Branch"
  git branch --show-current 2>/dev/null || true
  echo
  echo "## Last commit"
  git log -1 --oneline 2>/dev/null || true
  echo
  echo "## Status"
  git status --short 2>/dev/null || true
  echo
  echo "## Diff stat"
  git diff --stat 2>/dev/null || true
} > "$DIR/git.txt"

{
  rustc --version 2>/dev/null || true
  cargo --version 2>/dev/null || true
  node --version 2>/dev/null || true
  npm --version 2>/dev/null || true
} > "$DIR/tool-versions.txt"

cargo metadata --no-deps > "$DIR/cargo-metadata.json" 2> "$DIR/cargo-metadata.err" || true
cargo check --workspace > "$DIR/cargo-check.log" 2>&1 || true

if [[ -f "apps/desktop/package.json" ]]; then
  npm --prefix apps/desktop run build > "$DIR/desktop-build.log" 2>&1 || true
fi

./scripts/secret-scan-basic.sh > "$DIR/secret-scan.log" 2>&1 || true

echo "Debug bundle created: $DIR"
echo "Share selected logs from this folder when asking for help. Do not share secrets."
