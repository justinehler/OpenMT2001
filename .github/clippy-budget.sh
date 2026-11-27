#!/usr/bin/env bash
# Runs clippy over every target and fails when the number of distinct warnings
# exceeds the budget in .github/clippy-budget. When the count drops, lower the
# budget in the same pull request so it cannot creep back up.
#
#   .github/clippy-budget.sh [+TOOLCHAIN]
set -uo pipefail
cd "$(dirname "$0")/.."
toolchain=()
[[ "${1:-}" == +* ]] && toolchain=("$1")
budget=$(tr -dc '0-9' < .github/clippy-budget)
log=$(mktemp)
cargo "${toolchain[@]}" clippy --locked --features native,capture --all-targets \
  --message-format short --color never > "$log" 2>&1
status=$?
if ((status != 0)) && ! grep -q 'Finished' "$log"; then
  cat "$log"
  echo "::error::clippy did not finish"
  exit "$status"
fi
# One line per distinct diagnostic; duplicates from lib and lib-test builds collapse.
count=$(grep -E '^[^ ]+:[0-9]+:[0-9]+: warning: ' "$log" | sort -u | wc -l)
errors=$(grep -E '^[^ ]+:[0-9]+:[0-9]+: error: ' "$log" | sort -u)
if [[ -n "$errors" ]]; then
  echo "$errors"
  echo "::error::clippy reported errors"
  exit 1
fi
echo "Most frequent clippy warnings:"
grep -E '^[^ ]+:[0-9]+:[0-9]+: warning: ' "$log" | sort -u | sed -E 's/^.*: warning: //; s/`[^`]*`/_/g' \
  | sort | uniq -c | sort -rn | head -15
echo "clippy warnings: $count (budget $budget)"
if ((count > budget)); then
  echo "New warnings in this change:"
  base=FETCH_HEAD
  git fetch -q --depth=1 origin "${GITHUB_BASE_REF:-main}" 2>/dev/null || base=HEAD~1
  git diff --name-only --diff-filter=AM "$base" HEAD 2>/dev/null | grep '\.rs$' \
    | while read -r file; do grep -F "$file:" "$log" | grep -E "^[^ ]+:[0-9]+:[0-9]+: warning: "; done | sort -u
  echo "::error::clippy warnings rose to $count, over the budget of $budget. Fix the new warnings."
  exit 1
fi
if ((count < budget)); then
  echo "::notice::clippy warnings fell to $count. Lower .github/clippy-budget from $budget to $count."
fi
