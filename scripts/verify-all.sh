#!/usr/bin/env bash
set -euo pipefail

echo "== RepoDesk full verify =="

cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace

if [[ -f "apps/desktop/package.json" ]]; then
  npm --prefix apps/desktop run build
fi

./scripts/secret-scan-basic.sh

echo "OK: full verification passed"
