#!/usr/bin/env bash
set -euo pipefail

# Frontend daily-loop E2E smoke (Playwright + mocked Tauri IPC).
# Runs everywhere — macOS dev machines and headless CI. Boots the real React UI
# against the Vite dev server with a fake IPC layer; no Rust backend required.
# Real-backend native E2E lives in ./scripts/e2e-native.sh (Linux/CI only).

cd "$(dirname "$0")/.."

echo "== RepoDesk E2E smoke (Playwright, mock IPC) =="

pnpm --dir apps/desktop install --frozen-lockfile
pnpm --dir apps/desktop exec playwright install chromium
pnpm --dir apps/desktop e2e

echo "OK: Playwright daily-loop smoke passed"
