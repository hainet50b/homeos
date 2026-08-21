#!/usr/bin/env pwsh
# .ralph/gate.ps1 — the repo's pass gate. A PRD task is checked off only
# after this exits 0, and every run integration re-runs it. Keep gate.sh
# behaviorally identical.
$ErrorActionPreference = 'Stop'

cargo fmt --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo clippy -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo test
exit $LASTEXITCODE
