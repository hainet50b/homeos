#!/usr/bin/env bash
set -euo pipefail

MAX_ITERATIONS=${1:-10}
PROMPT_FILE="prompt.md"
SLEEP_SECONDS=2

METRICS_FILE="metrics.csv"

total_duration_ms=0
total_input_tokens=0
total_output_tokens=0
total_cost=0

if [[ ! -s "$METRICS_FILE" ]]; then
  echo "timestamp,commit,message,duration_s,input_tokens,output_tokens,cost_usd" > "$METRICS_FILE"
fi

echo "=== Ralph Loop ==="
echo "Max iterations: $MAX_ITERATIONS"
echo ""

for ((i = 1; i <= MAX_ITERATIONS; i++)); do
  echo "--- Iteration $i / $MAX_ITERATIONS ---"

  json=$(claude --dangerously-skip-permissions -p "$(cat "$PROMPT_FILE")" --output-format json 2>&1) || true

  result=$(echo "$json" | jq -r '.result // empty')
  duration_ms=$(echo "$json" | jq -r '.duration_ms // 0')
  input_tokens=$(echo "$json" | jq -r '.usage.input_tokens // 0')
  output_tokens=$(echo "$json" | jq -r '.usage.output_tokens // 0')
  cost=$(echo "$json" | jq -r '.total_cost_usd // 0')

  duration_s=$(echo "$duration_ms / 1000" | bc)
  timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  commit=$(git rev-parse --short HEAD)
  message=$(git log -1 --format=%s)
  echo "${timestamp},${commit},\"${message}\",${duration_s},${input_tokens},${output_tokens},${cost}" >> "$METRICS_FILE"

  total_duration_ms=$((total_duration_ms + duration_ms))
  total_input_tokens=$((total_input_tokens + input_tokens))
  total_output_tokens=$((total_output_tokens + output_tokens))
  total_cost=$(echo "$total_cost + $cost" | bc)

  echo "$result"
  echo ""
  cost_display=$(printf "%.2f" "$cost")
  echo "--- Iteration $i completed in ${duration_s}s | in: ${input_tokens} out: ${output_tokens} cost: \$${cost_display} ---"
  echo ""

  if [[ "$result" == *"<promise>COMPLETE</promise>"* ]]; then
    total_duration_s=$(echo "$total_duration_ms / 1000" | bc)
    git add "$METRICS_FILE" && git commit -m "Add metrics.csv for Ralph Loop run"
    total_cost_display=$(printf "%.2f" "$total_cost")
    echo "=== All tasks complete! (total: ${total_duration_s}s | in: ${total_input_tokens} out: ${total_output_tokens} cost: \$${total_cost_display}) ==="
    exit 0
  fi

  sleep "$SLEEP_SECONDS"
done

git add "$METRICS_FILE" && git commit -m "Add metrics.csv for Ralph Loop run (incomplete)"
total_duration_s=$(echo "$total_duration_ms / 1000" | bc)
total_cost_display=$(printf "%.2f" "$total_cost")
echo "=== Reached max iterations ($MAX_ITERATIONS) without completing all tasks (total: ${total_duration_s}s | in: ${total_input_tokens} out: ${total_output_tokens} cost: \$${total_cost_display}) ==="
exit 1
