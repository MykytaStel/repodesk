#!/usr/bin/env bash
set -euo pipefail

echo "== RepoDesk desktop smoke check =="

cargo check -p repodesk-desktop

if [[ -f "apps/desktop/package.json" ]]; then
  npm --prefix apps/desktop run build
fi

echo "OK: desktop smoke check passed"
echo "Start app with: ./scripts/dev-desktop.sh"
