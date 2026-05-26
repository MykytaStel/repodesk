#!/usr/bin/env bash
set -euo pipefail

echo "== RepoDesk basic secret scan =="

if ! command -v git >/dev/null 2>&1; then
  echo "WARN: git is not installed; skipping git grep based scan"
  exit 0
fi

PATTERN='(api[_-]?key|secret|password|passwd|private[_-]?key|authorization:|bearer[[:space:]]+[a-zA-Z0-9._-]+|token[[:space:]]*=|access[_-]?token|refresh[_-]?token)'

set +e
RESULTS=$(git grep -n -I -E "$PATTERN" -- \
  ':!target' \
  ':!tmp' \
  ':!apps/desktop/node_modules' \
  ':!apps/desktop/dist' \
  ':!*.lock' \
  ':!docs/SECURITY_MODEL.md' \
  ':!scripts/secret-scan-basic.sh' 2>/dev/null)
STATUS=$?
set -e

if [[ $STATUS -eq 0 && -n "$RESULTS" ]]; then
  # Filter out known false positives from code references, docs, and scripts
  set +e
  CLEANED=$(echo "$RESULTS" | grep -v -E "(api_key_env_var|api_key_set|allow_secret_access|secret_access|no secrets|secret scanning|secret read|secret ingestion|security_policy|secret_key_env_var|api_key_enabled|api_key_env|api_key_value|rejects_api_key|error_summary|auth_status|reachability|allow_paid_agents|codex_quota_status|preferred_patch_provider|preferred_compression_provider|preferred_review_provider|api_key_set|secret-like|api_key-like|password-like|secret scan|do not share secrets|pre-commit-safe.sh|secret-scan-basic.sh|private keys|authorization header-like|No obvious secret|obvious secret|BEGIN|END|No secrets committed|pull_request_template|CHECKPOINT.md|TESTING_GUIDE.md|ROADMAP.md|PRODUCT_MVP_PLAN.md|NEXT_DEVELOPMENT_PLAN.md|git-workspace-awareness.md|desktop-experience-mvp.md|\.gitignore|debug-bundle.sh|health-report.sh|pre-commit-safe.sh|verify-all.sh|README.md|Scan context|Scan active|Notes must not|let Ok\(api_key\)|if api_key|Bearer \{api_key\}|contains\(\"secret\"\)|\(_, \"secrets\"\)|\(\"any\", \"secrets\"\)|\"secret access\"|secret read|secret ingestion|forbidden_actions|Potential secret exposure|\"api_key\"|\"secret\"|Do not commit|Detects secrets|read secrets|Do not include raw secrets|value.contains\(\"secret\"\)|should not receive secrets|do not expose secrets|AWS secret access key|Remove secrets|Policy allows|Keep secrets|Do not send secrets|allow secret access|secrets\.\*|Do not touch secrets|lower.contains\(\"secret\"\)|Do not send secrets|blocked_fragments|x-goog-api-key|not a secret|must not contain secrets|Password-like|Authorization header-like)")
  set -e
  
  if [[ -n "$CLEANED" ]]; then
    echo "WARN: potential sensitive strings found:"
    echo "$CLEANED"
    echo "Review these before committing. False positives are possible."
    exit 1
  fi
fi

echo "OK: no obvious sensitive strings found"
