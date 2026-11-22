#!/usr/bin/env bash
# Audit every committed Cargo.lock with cargo-audit, not a hand-kept list.
# Used by the CI "Security Audit" job and by the planted controls in
# ci-controls.toml (audit-*). Usage: tools/audit-lockfiles.sh <cargo-audit>
#
# In a git checkout the lockfiles are `git ls-files '*Cargo.lock'`. plant runs
# controls in a `git archive` export with no .git, which holds exactly the
# tracked files, so there the same set is found with `find`. ci-controls/ is
# skipped in both modes: it holds the planted lockfiles the controls overlay.
set -uo pipefail
audit="${1:?usage: $0 <path to cargo-audit>}"
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  locks=$(git ls-files -- '*Cargo.lock' ':!ci-controls/**')
else
  locks=$(find . -name Cargo.lock -not -path '*/target/*' -not -path './ci-controls/*' | sed 's|^\./||' | sort)
fi
n=$(printf '%s\n' "$locks" | grep -c . || true)
[ "$n" -gt 0 ] || { echo "::error::no Cargo.lock found to audit" >&2; exit 1; }
echo "auditing $n lockfiles"
status=0
for lock in $locks; do
  echo "::group::cargo audit --file $lock"
  "$audit" audit --file "$lock" || status=1
  echo "::endgroup::"
done
exit "$status"
