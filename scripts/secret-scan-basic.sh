#!/usr/bin/env bash
set -euo pipefail

echo "== RepoDesk basic secret scan =="

if ! command -v git >/dev/null 2>&1; then
  echo "WARN: git is not installed; skipping git grep based scan"
  exit 0
fi

# Detect literal secret *values*, not identifiers. Flagging variable names like
# `api_key: String` or `env("OPENAI_API_KEY")` is noise; what matters is a real
# secret value committed to the tree. Each alternative below targets a value:
#   - a secret-named field assigned a long quoted literal
#   - a long bare token assigned after `=`
#   - AWS access key ids, private-key blocks, bearer tokens
#   - well-known provider token prefixes (Slack/Stripe/GitHub)
PATTERN='(api[_-]?key|secret|token|password|passwd|access[_-]?token|refresh[_-]?token)["'"'"']?[[:space:]]*[:=][[:space:]]*["'"'"'][A-Za-z0-9._/+=-]{16,}["'"'"']'
PATTERN+='|[A-Z_]*(API_KEY|SECRET|TOKEN|PASSWORD)[A-Z_]*=[[:space:]]*[A-Za-z0-9._/+=-]{24,}'
PATTERN+='|AKIA[0-9A-Z]{16}'
PATTERN+='|-----BEGIN [A-Z ]*PRIVATE KEY-----'
PATTERN+='|bearer[[:space:]]+[A-Za-z0-9._-]{20,}'
PATTERN+='|xox[baprs]-[A-Za-z0-9-]{10,}|sk_live_[0-9A-Za-z]{16,}|ghp_[0-9A-Za-z]{20,}|github_pat_[0-9A-Za-z_]{20,}'

set +e
# Excluded paths: build output, lockfiles, the secret-scanner modules + configs
# themselves (they contain secret *patterns* by design), and test fixtures (they
# embed fake example secrets to exercise the scanners).
RESULTS=$(git grep -n -I -E "$PATTERN" -- \
  ':!target' \
  ':!tmp' \
  ':!apps/desktop/node_modules' \
  ':!apps/desktop/dist' \
  ':!*.lock' \
  ':!crates/repodesk-core/src/security.rs' \
  ':!crates/repodesk-core/src/safety.rs' \
  ':!crates/repodesk-core/tests' \
  ':!scripts/secret-scan-basic.sh' \
  ':!.gitleaks.toml' 2>/dev/null)
STATUS=$?
set -e

# AWS publishes one canonical access-key-shaped value for documentation and
# scanner tests. Keep the AWS rule strict, but remove only that exact public
# sentinel before deciding whether a matching line still contains a secret.
# If another secret is present on the same line it remains in RESULTS.
if [[ $STATUS -eq 0 && -n "$RESULTS" ]]; then
  PUBLIC_AWS_EXAMPLE='AK''IAIOSFODNN7EXAMPLE'
  FILTERED_RESULTS=""
  while IFS= read -r line; do
    sanitized=${line//$PUBLIC_AWS_EXAMPLE/}
    if printf '%s\n' "$sanitized" | grep -E -q "$PATTERN"; then
      FILTERED_RESULTS+="$line"$'\n'
    fi
  done <<< "$RESULTS"
  RESULTS=${FILTERED_RESULTS%$'\n'}
fi

if [[ -n "$RESULTS" ]]; then
  echo "WARN: potential hardcoded secret values found:"
  echo "$RESULTS"
  echo "Review these before committing. False positives are possible."
  exit 1
fi

echo "OK: no obvious hardcoded secret values found"
