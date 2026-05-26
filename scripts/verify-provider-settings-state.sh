#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all -- --check
cargo check -p repodesk-desktop
cargo test -p repodesk-desktop
npm --prefix apps/desktop run build

echo "Provider settings state verification passed."
