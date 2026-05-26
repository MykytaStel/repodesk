#!/usr/bin/env bash
set -euo pipefail

echo "== RepoDesk next development step =="
echo

echo "Current recommended sequence:"
echo "1. feature/stability-optimization"
echo "2. feature/product-workflow-hardening"
echo "3. feature/sqlite-state-store"
echo "4. feature/ollama-health-runtime"
echo "5. feature/security-hardening"
echo "6. feature/desktop-packaging"
echo

echo "Run now:"
echo "  ./scripts/verify-fast.sh"
echo "  ./scripts/repo-state.sh"
echo "  ./scripts/health-report.sh"
echo

echo "Read: docs/PRODUCT_MVP_PLAN.md"
