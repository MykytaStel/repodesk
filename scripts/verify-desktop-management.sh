#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all -- --check
cargo check -p repodesk-desktop
cargo test -p repodesk-desktop

if [ -d "apps/desktop/node_modules" ]; then
  (cd apps/desktop && npm run build)
else
  echo "Skipping npm build because apps/desktop/node_modules does not exist. Run ./scripts/dev-desktop.sh once to install dependencies."
fi

echo "Desktop management verification passed."
