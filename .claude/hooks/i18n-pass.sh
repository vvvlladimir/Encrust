#!/usr/bin/env bash
# Asks for the i18n-editor pass once a turn ends with catalogues changed since the last pass.
# `check` is the Stop hook; `mark` records the current catalogues as passed (per clone).
set -euo pipefail

root=${CLAUDE_PROJECT_DIR:-$PWD}
cd "$root"
marker="$(git rev-parse --git-dir)/i18n-passed"
base=$(git merge-base HEAD main 2>/dev/null || git rev-parse HEAD)
diff=$(git diff "$base" -- assets/i18n)
digest=$(printf '%s' "$diff" | shasum | cut -d' ' -f1)

case "${1:-check}" in
mark)
  printf '%s\n' "$digest" >"$marker"
  exit 0
  ;;
check) ;;
*) exit 0 ;;
esac

command -v jq >/dev/null || exit 0
[ "$(jq -r '.stop_hook_active // false')" = "true" ] && exit 0
[ -z "$diff" ] && exit 0
[ "$(cat "$marker" 2>/dev/null)" = "$digest" ] && exit 0

jq -n '{decision: "block", reason: "assets/i18n changed since the last language pass. Run the i18n-editor agent on the changed messages, relay its table, then finish."}'
