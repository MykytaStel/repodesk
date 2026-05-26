#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all -- --check
cargo check -p repodesk-core
cargo check -p repodesk-desktop
cargo test -p repodesk-desktop

cd apps/desktop
if [ ! -d node_modules ]; then
  npm install
fi
npm run build
