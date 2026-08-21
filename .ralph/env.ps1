# .ralph/env.ps1 — repo-level knobs for ralph.ps1, dot-sourced before the run
# and before the integration gate. Values already set in the invoking
# environment win over this file. Keep env.sh behaviorally identical.

# Default agent command — the model choice lives here:
$env:RALPH_CMD = 'claude --model opus --dangerously-skip-permissions -p'

# Shared cache so per-run worktrees don't rebuild from scratch:
$env:CARGO_TARGET_DIR = Join-Path (git rev-parse --show-toplevel) 'target'
