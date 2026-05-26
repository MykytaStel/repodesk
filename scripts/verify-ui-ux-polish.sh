#!/usr/bin/env bash
set -euo pipefail

if [[ ! -d apps/desktop ]]; then
  echo "ERROR: apps/desktop not found"
  exit 1
fi

echo "==> Checking desktop frontend build"
npm --prefix apps/desktop run build

echo "==> Checking desktop Rust crate"
cargo check -p repodesk-desktop

echo "==> UI/UX polish verification passed"
