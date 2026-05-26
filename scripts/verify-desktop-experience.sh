#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all -- --check
cargo check -p repodesk-desktop
cargo test -p repodesk-desktop

cd apps/desktop
npm run build
