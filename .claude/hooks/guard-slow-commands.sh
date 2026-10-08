#!/usr/bin/env bash
# Denies the commands that cost ten minutes: a full-workspace cargo run, and anything that
# moves the compiler pin, which throws every cached artifact away. See
# .claude/rules/workflow.md. Prefix a command with ENCRUST_FULL_CHECK=1 to run it anyway.
set -euo pipefail

payload=$(cat)
command=$(printf '%s' "$payload" | jq -r '.tool_input.command // ""')
file=$(printf '%s' "$payload" | jq -r '.tool_input.file_path // ""')

deny() {
  jq -n --arg reason "$1" '{
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason: $reason
    }
  }'
  exit 0
}

pin_reason='rust-toolchain.toml pins the compiler on purpose: changing it, or installing another toolchain, invalidates every cached artifact and costs a ten-minute rebuild. A failing check is never answered by moving the pin. Ask the user if the pin really has to move.'
scope_reason='A workspace-wide cargo run rebuilds the whole graph. Check only the crate you touched: cargo test -p <crate>, cargo clippy -p <crate> --all-targets -- -D warnings. The three workspace checks run once, at the end, and only when the user asks for them; prefix that run with ENCRUST_FULL_CHECK=1.'

case "$file" in
*rust-toolchain.toml) deny "$pin_reason" ;;
esac

case "$command" in
*ENCRUST_FULL_CHECK=1*) exit 0 ;;
esac

if printf '%s' "$command" | grep -qE 'rustup +(update|default|toolchain +install|override)'; then
  deny "$pin_reason"
fi

if printf '%s' "$command" | grep -qE 'rust-toolchain\.toml' &&
  printf '%s' "$command" | grep -qE '(sed +-i|tee|>>?|cp .* rust-toolchain)'; then
  deny "$pin_reason"
fi

if printf '%s' "$command" |
  grep -qE 'cargo[^|;&]*\b(test|clippy|build|check|bench)\b[^|;&]*(--workspace|--all-targets)' &&
  ! printf '%s' "$command" | grep -qE '(-p|--package) +[A-Za-z]'; then
  deny "$scope_reason"
fi

exit 0
