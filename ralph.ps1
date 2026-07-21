#!/usr/bin/env pwsh
# ralph.ps1 — Ralph Loop driver (PowerShell).
#
# Usage:
#   ./ralph.ps1 [<max-iterations>]
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

[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [int]$MaxIterations = 10
)

$ErrorActionPreference = 'Stop'

$PromptFile = 'prompt.md'
$SleepSeconds = 2

if (-not (Test-Path -LiteralPath $PromptFile)) {
    Write-Error "error: $PromptFile not found in $(Get-Location)."
    exit 1
}

if (-not (Get-Command claude -ErrorAction SilentlyContinue)) {
    Write-Error "error: 'claude' CLI not found on PATH."
    exit 1
}

Write-Host "=== Ralph Loop ==="
Write-Host "Max iterations: $MaxIterations"
Write-Host ""

$PromptContent = Get-Content -LiteralPath $PromptFile -Raw

for ($i = 1; $i -le $MaxIterations; $i++) {
    Write-Host "--- Iteration $i / $MaxIterations ---"

    $jsonText = & claude --dangerously-skip-permissions -p $PromptContent --output-format json 2>&1 | Out-String

    if ($LASTEXITCODE -ne 0) {
        Write-Host "error: claude invocation failed; exiting." -ForegroundColor Red
        Write-Host $jsonText
        exit 1
    }

    try {
        $data = $jsonText | ConvertFrom-Json
    }
    catch {
        Write-Host "error: failed to parse claude output as JSON; exiting." -ForegroundColor Red
        Write-Host $jsonText
        exit 1
    }

    $result = if ($data.PSObject.Properties['result']) { [string]$data.result } else { '' }
    $durationMs = if ($data.PSObject.Properties['duration_ms']) { [int64]$data.duration_ms } else { 0 }
    $durationS = [int]($durationMs / 1000)

    Write-Host $result
    Write-Host ""
    Write-Host "--- Iteration $i completed in ${durationS}s ---"
    Write-Host ""

    if ($result -like '*<promise>COMPLETE</promise>*') {
        Write-Host "=== All tasks complete ==="
        exit 0
    }

    Start-Sleep -Seconds $SleepSeconds
}

Write-Host "=== Reached max iterations ($MaxIterations) without completion ==="
exit 1
