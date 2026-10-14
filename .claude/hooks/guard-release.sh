#!/usr/bin/env bash
# Denies every command that changes a release or pushes: only the maintainer publishes, from
# their own shell. The /release-notes skill writes a file and hands them the command.
set -euo pipefail

command=$(jq -r '.tool_input.command // ""')

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

reason='Releases and pushes are the maintainer'"'"'s alone. Write the change to a file and give the user the command to run it themselves.'

if printf '%s' "$command" | grep -qE 'gh +release +(create|edit|upload|delete)'; then
  deny "$reason"
fi

# `gh api` turns into a POST on its own once it is given a field or an input.
if printf '%s' "$command" | grep -qE 'gh +api\b' &&
  printf '%s' "$command" | grep -qE '(-X|--method)[ =]*(POST|PATCH|PUT|DELETE)|(^| )(-f|-F|--field|--raw-field|--input)\b'; then
  deny "$reason"
fi

if printf '%s' "$command" | grep -qE 'git +push\b'; then
  deny "$reason"
fi

exit 0
