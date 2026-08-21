#!/usr/bin/env bash
# .ralph/gate.sh — the repo's pass gate. A PRD task is checked off only
# after this exits 0, and every run integration re-runs it. Keep gate.ps1
# behaviorally identical.
set -euo pipefail

cargo fmt --check
cargo clippy -- -D warnings
cargo test
