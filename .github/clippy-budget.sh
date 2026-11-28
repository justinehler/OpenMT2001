#!/usr/bin/env bash
# Runs clippy over every target. A pull request fails when clippy warns on a
# line it adds or modifies. Any run fails when the number of distinct warnings
# exceeds the budget in .github/clippy-budget by more than a small slack. When
# the count drops, lower the budget so it cannot creep back up.
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

# Pull requests: fail on warnings in lines this change adds or modifies. This
# does not depend on what else merges meanwhile, so two passing PRs cannot
# combine into a failure on main.
if [[ -n "${GITHUB_BASE_REF:-}" ]]; then
  git fetch -q --depth=1 origin "$GITHUB_BASE_REF" || { echo "::error::cannot fetch $GITHUB_BASE_REF"; exit 1; }
  # HEAD is the merge of the PR into the base, so this diff is exactly the PR.
  # Lines that only moved (an added line matching a removed line anywhere in
  # the diff, ignoring whitespace, visibility and trailing commas) don't count:
  # a pure-move refactor carries its existing warnings with it.
  added=$(mktemp)
  git diff -U0 --no-color FETCH_HEAD HEAD -- '*.rs' | awk '
    function norm(t) {
      sub(/^[+-]/, "", t); gsub(/[ \t]+/, " ", t); sub(/^ /, "", t); sub(/ $/, "", t)
      sub(/^pub(\([a-z]+\))? /, "", t); sub(/,$/, "", t); return t
    }
    /^\+\+\+ b\// { file = substr($0, 7); next }
    /^---/ { next }
    /^@@/ { split($3, r, ","); line = substr(r[1], 2); next }
    /^-/ { removed[norm($0)]++; next }
    /^\+/ { added[++n] = file ":" line; text[n] = norm($0); line++ }
    END { for (i = 1; i <= n; i++) {
            if (text[i] != "" && removed[text[i]] > 0) { removed[text[i]]--; continue }
            print added[i] } }' > "$added" || { echo "::error::cannot attribute warnings to changed lines"; exit 1; }
  # The changed-line list goes through a file: as an awk -v argument it can
  # exceed the kernel's per-argument limit on large diffs and fail silently.
  new_warnings=$(grep -E '^[^ ]+:[0-9]+:[0-9]+: warning: ' "$log" | sort -u | awk '
    NR == FNR { changed[$0] = 1; next }
    { split($0, p, ":"); if ((p[1] ":" p[2]) in changed) print }' "$added" -)
  if [[ -n "$new_warnings" ]]; then
    echo "Warnings on lines this change adds or modifies:"
    echo "$new_warnings"
    echo "::error::this change adds $(echo "$new_warnings" | wc -l) clippy warning(s). Fix them."
    exit 1
  fi
fi

# Backstop against drift that line attribution misses (for example a new
# warning at an untouched call site). Lower the budget as the count falls.
slack=20
if ((count > budget + slack)); then
  echo "::error::clippy warnings rose to $count, more than $slack over the budget of $budget."
  exit 1
fi
if ((count < budget)); then
  echo "::notice::clippy warnings fell to $count. Lower .github/clippy-budget from $budget to $count."
elif ((count > budget)); then
  echo "::warning::clippy warnings are $count, over the budget of $budget."
fi
