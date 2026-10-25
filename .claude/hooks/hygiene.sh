#!/usr/bin/env bash
# After an edit to a Rust source, holds that file to the size limits by the ratchet in
# `cargo xtask hygiene`. Uses the xtask binary already built; without one it stays silent.
set -euo pipefail

command -v jq >/dev/null || exit 0
root=${CLAUDE_PROJECT_DIR:-$PWD}
file=$(jq -r '.tool_input.file_path // ""')
case "$file" in
*.rs) ;;
*) exit 0 ;;
esac

xtask="$root/target/debug/xtask"
[ -x "$xtask" ] || exit 0

if ! output=$("$xtask" hygiene --root "$root" --file "$file" 2>&1); then
  printf '%s\n' "$output" >&2
  exit 2
fi
