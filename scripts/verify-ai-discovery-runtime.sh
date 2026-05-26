#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all -- --check
cargo check --workspace
cargo test -p repodesk-core ai_discovery -- --nocapture
cargo check -p repodesk-desktop
npm --prefix apps/desktop run build

echo "AI discovery runtime verification passed."
