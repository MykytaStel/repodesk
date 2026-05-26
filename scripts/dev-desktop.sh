#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/../apps/desktop"

if [ ! -d node_modules ]; then
  echo "Installing desktop dependencies..."
  npm install
fi

npm run desktop:dev
