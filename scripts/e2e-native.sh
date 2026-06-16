#!/usr/bin/env bash
set -euo pipefail

# Real-backend Tauri E2E (tauri-driver + WebdriverIO). Linux only — tauri-driver
# does not support macOS. On a Mac, use ./scripts/e2e-smoke.sh instead.
# Assumes a release build already exists; pass --build to build it here.

cd "$(dirname "$0")/.."

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "SKIP: native E2E needs Linux (tauri-driver has no macOS support)." >&2
  echo "      Run ./scripts/e2e-smoke.sh for the Playwright mock-IPC smoke." >&2
  exit 0
fi

echo "== RepoDesk native E2E (tauri-driver + WebdriverIO) =="

command -v tauri-driver >/dev/null || { echo "Missing tauri-driver: cargo install tauri-driver --locked" >&2; exit 1; }
command -v WebKitWebDriver >/dev/null || { echo "Missing WebKitWebDriver: install webkit2gtk-driver" >&2; exit 1; }

if [[ "${1:-}" == "--build" ]]; then
  pnpm --dir apps/desktop install --frozen-lockfile
  pnpm --dir apps/desktop tauri build
fi

pnpm --dir apps/desktop/e2e-native install

REPODESK_HOME="${REPODESK_HOME:-$(mktemp -d)}"
export REPODESK_HOME
echo "Using throwaway REPODESK_HOME=$REPODESK_HOME"

if command -v xvfb-run >/dev/null; then
  xvfb-run -a pnpm --dir apps/desktop/e2e-native test
else
  pnpm --dir apps/desktop/e2e-native test
fi

echo "OK: native E2E smoke passed"
