#!/usr/bin/env pwsh
# ralph.ps1 — Ralph Loop driver (PowerShell).
#
# Usage:
#   ./ralph.ps1 [<max-iterations>]
#
# Defaults:
#   max-iterations: 10
#
# Environment variables:
#   RALPH_REPORT_ROTATE_BYTES   size threshold for reports/report.html
#                               rotation (default: 524288 = 512 KB)
#
# Behavior:
#   - On each iteration: rotate reports/report.html if it exceeds the size
#     threshold (the current file becomes reports/report-<UTC>.html and a
#     fresh one is started on the next iteration), then invoke `claude`
#     with prompt.md as the prompt.
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
$ReportsDir = 'reports'
$ReportFile = Join-Path $ReportsDir 'report.html'
$RotateBytes = if ($env:RALPH_REPORT_ROTATE_BYTES) { [int64]$env:RALPH_REPORT_ROTATE_BYTES } else { 524288 }
$SleepSeconds = 2

New-Item -ItemType Directory -Force -Path $ReportsDir | Out-Null

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
Write-Host "Rotation threshold: $RotateBytes bytes"
Write-Host ""

$PromptContent = Get-Content -LiteralPath $PromptFile -Raw

for ($i = 1; $i -le $MaxIterations; $i++) {
    Write-Host "--- Iteration $i / $MaxIterations ---"

    # Rotate report.html before the iteration if it exceeds the threshold.
    if (Test-Path -LiteralPath $ReportFile) {
        $size = (Get-Item -LiteralPath $ReportFile).Length
        if ($size -gt $RotateBytes) {
            $ts = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssZ')
            $archived = Join-Path $ReportsDir "report-$ts.html"
            Move-Item -LiteralPath $ReportFile -Destination $archived
            Write-Host "Rotated $ReportFile ($size bytes) -> $archived"
        }
    }

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
