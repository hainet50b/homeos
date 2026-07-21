#!/usr/bin/env bash
# ralph.sh — Ralph Loop driver.
#
# Usage:
#   ./ralph.sh [<max-iterations>]
#
# Defaults:
#   max-iterations: 10
#
# Behavior:
#   - On each iteration: invoke `claude` with prompt.md as the prompt.
#   - claude failure exits immediately (no retry); transient failures are
#     rare and retrying wastes time / tokens.
#   - Detects `<promise>COMPLETE</promise>` in claude's output and exits 0
#     when seen.
#   - Reaching max-iterations without completion exits 1.
#
# Dependencies:
#   - claude (Anthropic CLI)
#   - jq

set -euo pipefail

MAX_ITERATIONS=${1:-10}
PROMPT_FILE="prompt.md"
SLEEP_SECONDS=2

if [[ ! -f "$PROMPT_FILE" ]]; then
  echo "error: $PROMPT_FILE not found in $(pwd)." >&2
  exit 1
fi

command -v claude >/dev/null 2>&1 || { echo "error: 'claude' CLI not found on PATH." >&2; exit 1; }
command -v jq     >/dev/null 2>&1 || { echo "error: 'jq' not found on PATH."     >&2; exit 1; }

echo "=== Ralph Loop ==="
echo "Max iterations: $MAX_ITERATIONS"
echo ""

for ((i = 1; i <= MAX_ITERATIONS; i++)); do
  echo "--- Iteration $i / $MAX_ITERATIONS ---"

  if ! json=$(claude --dangerously-skip-permissions -p "$(cat "$PROMPT_FILE")" --output-format json 2>&1); then
    echo "error: claude invocation failed; exiting." >&2
    echo "$json" >&2
    exit 1
  fi

  result=$(echo "$json" | jq -r '.result // empty')
  duration_ms=$(echo "$json" | jq -r '.duration_ms // 0')
  duration_s=$(( duration_ms / 1000 ))

  echo "$result"
  echo ""
  echo "--- Iteration $i completed in ${duration_s}s ---"
  echo ""

  if [[ "$result" == *"<promise>COMPLETE</promise>"* ]]; then
    echo "=== All tasks complete ==="
    exit 0
  fi

  sleep "$SLEEP_SECONDS"
done

echo "=== Reached max iterations ($MAX_ITERATIONS) without completion ==="
exit 1
