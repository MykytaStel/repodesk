#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all -- --check
cargo check --workspace
cargo test -p repodesk-desktop
npm --prefix apps/desktop run build

echo "Product Workflow MVP verification passed."
