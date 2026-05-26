#!/usr/bin/env bash
set -euo pipefail

echo "== RepoDesk repository state =="
echo

echo "Branch:"
git branch --show-current 2>/dev/null || true
echo

echo "Last commit:"
git log -1 --oneline 2>/dev/null || true
echo

echo "Status:"
git status --short 2>/dev/null || true
echo

echo "Diff stat:"
git diff --stat 2>/dev/null || true
echo

echo "Staged diff stat:"
git diff --cached --stat 2>/dev/null || true
