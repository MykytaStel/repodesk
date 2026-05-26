#!/usr/bin/env bash
set -euo pipefail

./scripts/verify-fast.sh
./scripts/secret-scan-basic.sh

echo "OK: safe pre-commit checks passed"
