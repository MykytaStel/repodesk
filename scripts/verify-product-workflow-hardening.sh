#!/usr/bin/env bash
set -euo pipefail

echo "== RepoDesk product workflow hardening verify =="

cargo fmt --all -- --check
cargo check --workspace

if [ -d "apps/desktop/node_modules" ]; then
  npm --prefix apps/desktop run build
else
  npm --prefix apps/desktop install
  npm --prefix apps/desktop run build
fi

echo "OK: product workflow hardening verification passed"
