#!/usr/bin/env bash
set -euo pipefail

echo "== RepoDesk fast verify =="

cargo fmt --all -- --check
cargo check --workspace

if [[ -f "apps/desktop/package.json" ]]; then
  npm --prefix apps/desktop run build
fi

echo "OK: fast verification passed"
