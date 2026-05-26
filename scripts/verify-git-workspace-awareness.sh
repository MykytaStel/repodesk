#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all -- --check
cargo check --workspace
cargo test -p repodesk-core git_workspace -- --nocapture
cargo test -p repodesk-desktop
npm --prefix apps/desktop run build

echo "Git workspace awareness verification passed."
