#!/usr/bin/env pwsh
# ralph.ps1 — one-shot implementation runner; each run works in its own worktree.
#
# Usage:
#   ./ralph.ps1 [guidance...]
#   ./ralph.ps1 -Resume <run-id> [guidance...]
#
# Behavior:
#   - A new run gets a fresh worktree at .ralph/worktrees/<run-id> on branch
#     ralph/<run-id>, cut from the integration branch (whatever is checked
#     out when the run starts); the agent CLI runs once inside it. The
#     integration branch stays yours while the run is in flight: spec-layer
#     edits and new PRD tasks can land on it at any time.
#   - After the agent exits, the run is integrated deterministically: rebase
#     onto the integration branch, re-run the pass gate (.ralph/gate.ps1),
#     fast-forward the branch, remove the worktree. On a rebase conflict or a red gate nothing is
#     integrated and the worktree stays for inspection — this script never
#     resolves anything; escalation is the invoker's job.
#   - -Resume <run-id> starts a fresh agent context inside an existing run
#     worktree (interrupted or deliberately paused runs).
#   - Concurrent runs are supported: integration is serialized by a lock, and
#     per-task claims (.ralph/claims/<task-id>, owner-checked at integration)
#     keep two runs from landing the same task.
#
# Environment:
#   RALPH_CMD — agent command that reads the prompt on stdin; split on
#               whitespace. Default: claude --dangerously-skip-permissions -p
#   RALPH_LOCK_TIMEOUT — seconds a finished run waits for the integration
#               lock before giving up (default 900). A waiter never removes
#               the lock itself.
#   .ralph/env.ps1 — optional repo knobs, dot-sourced before the run and the
#               gate (agent/model via RALPH_CMD, shared build caches, …).
#               Values already set in the invoking environment win.

[CmdletBinding()]
param(
    [string]$Resume,
    [Parameter(Position = 0, ValueFromRemainingArguments = $true)]
    [string[]]$Guidance
)

$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath $PSScriptRoot

function Fail([int]$Code, [string]$Message) {
    [Console]::Error.WriteLine($Message)
    exit $Code
}

function Wait-IntegrationLock {
    $Timeout = if ($env:RALPH_LOCK_TIMEOUT) { [int]$env:RALPH_LOCK_TIMEOUT } else { 900 }
    $Waited = 0
    while ($true) {
        try {
            New-Item -ItemType Directory -Path $Lock -ErrorAction Stop | Out-Null
            break
        }
        catch {
            if ($Waited -ge $Timeout) {
                $Holder = if (Test-Path "$Lock/pid") { Get-Content "$Lock/pid" } else { 'unknown' }
                Fail 6 "INTEGRATE LOCKED OUT: $Lock held for ${Timeout}s (holder pid: $Holder); worktree kept at $Wt. if that integration is dead, remove the lock and rerun: Remove-Item -Recurse $Lock; ./ralph.ps1 -Resume $RunId"
            }
            Start-Sleep -Seconds 2
            $Waited += 2
        }
    }
    Set-Content -Path "$Lock/pid" -Value $PID
}

function Assert-Claims {
    $DoneIds = @(git -C $Wt diff "$Base..HEAD" -- PRD.md | Where-Object { $_ -match '^\+- \[x\] (T\d+):' } | ForEach-Object { $Matches[1] })
    foreach ($Id in $DoneIds) {
        $OwnerFile = ".ralph/claims/$Id/owner"
        $Owner = if (Test-Path -LiteralPath $OwnerFile) { (Get-Content -LiteralPath $OwnerFile -Raw).Trim() } else { '' }
        if ($Owner -ne "ralph/$RunId") {
            Fail 7 "INTEGRATE REFUSED: $Id was checked off without an owned claim; worktree kept at $Wt."
        }
    }
}

function Remove-OwnClaims {
    if (-not (Test-Path -LiteralPath '.ralph/claims')) { return }
    Get-ChildItem -LiteralPath '.ralph/claims' -Directory | ForEach-Object {
        $of = Join-Path $_.FullName 'owner'
        if ((Test-Path -LiteralPath $of) -and ((Get-Content -LiteralPath $of -Raw).Trim() -eq "ralph/$RunId")) {
            Remove-Item -LiteralPath $_.FullName -Recurse -Force
        }
    }
}

if (-not (Test-Path -LiteralPath '.ralph/prompt.md')) {
    Fail 1 "error: .ralph/prompt.md not found in $PSScriptRoot."
}
if (-not (Test-Path -LiteralPath '.ralph/gate.ps1')) {
    Fail 1 "error: .ralph/gate.ps1 not found in $PSScriptRoot — integration would have nothing to verify against."
}

$Base = git symbolic-ref --quiet --short HEAD
if ($LASTEXITCODE -ne 0 -or -not $Base) {
    Fail 1 "error: detached HEAD; check out the integration branch first."
}

$InvokerRalphCmd = $env:RALPH_CMD
if (Test-Path -LiteralPath '.ralph/env.ps1') { . ./.ralph/env.ps1 }
if ($InvokerRalphCmd) { $env:RALPH_CMD = $InvokerRalphCmd }
$RalphCmd = if ($env:RALPH_CMD) { $env:RALPH_CMD } else { 'claude --dangerously-skip-permissions -p' }

$Parts = $RalphCmd.Trim() -split '\s+'
$Exe = $Parts[0]
$CmdArgs = if ($Parts.Count -gt 1) { $Parts[1..($Parts.Count - 1)] } else { @() }
if (-not (Get-Command $Exe -ErrorAction SilentlyContinue)) {
    Fail 1 "error: '$Exe' not found on PATH."
}

if ($Resume) {
    $RunId = $Resume
    $Wt = ".ralph/worktrees/$RunId"
    if (-not (Test-Path -LiteralPath $Wt)) { Fail 1 "error: no run worktree at $Wt." }
}
else {
    $RunId = '{0}-{1}' -f (Get-Date -Format 'yyyyMMdd-HHmmss'), $PID
    $Wt = ".ralph/worktrees/$RunId"
    New-Item -ItemType Directory -Force -Path '.ralph/worktrees' | Out-Null
    git worktree add --quiet $Wt -b "ralph/$RunId" $Base
    if ($LASTEXITCODE -ne 0) { Fail 1 "error: git worktree add failed." }
    git -C $Wt branch --quiet --set-upstream-to=$Base
}

$Prompt = Get-Content -LiteralPath (Join-Path $Wt '.ralph/prompt.md') -Raw
if ($Guidance -and $Guidance.Count -gt 0) {
    $Prompt += "`n`n## Guidance from the invoker`n`n" + ($Guidance -join ' ')
    Write-Host "=== Implementation run $RunId (guided) ==="
}
else {
    Write-Host "=== Implementation run $RunId ==="
}
Write-Host ""

Push-Location -LiteralPath $Wt
$Prompt | & $Exe @CmdArgs
$AgentExit = $LASTEXITCODE
Pop-Location
if ($AgentExit -ne 0) {
    Fail 2 "RUN FAILED: agent exited non-zero; worktree kept at $Wt (./ralph.ps1 -Resume $RunId)."
}

$GatePath = Join-Path $Wt '.ralph/gate.ps1'
if (-not (Test-Path -LiteralPath $GatePath)) {
    Fail 4 "INTEGRATE FAILED: no pass gate at .ralph/gate.ps1; worktree kept at $Wt."
}

$Lock = '.ralph/integrate.lock'
Wait-IntegrationLock
try {
    git -C $Wt -c core.longpaths=true rebase --quiet $Base
    if ($LASTEXITCODE -ne 0) {
        git -C $Wt rebase --abort 2>$null
        Fail 3 "INTEGRATE CONFLICT: ralph/$RunId does not rebase onto ${Base}; worktree kept at $Wt for the invoker."
    }

    Push-Location -LiteralPath $Wt
    try {
        & ./.ralph/gate.ps1
        $GateExit = $LASTEXITCODE
    }
    catch {
        $GateExit = 1
    }
    finally {
        Pop-Location
    }
    if ($GateExit -ne 0) {
        Fail 4 "INTEGRATE GATE RED: rebased ralph/$RunId fails the gate; worktree kept at $Wt for the invoker."
    }

    Assert-Claims

    $Current = git symbolic-ref --quiet --short HEAD
    if ($Current -ne $Base) {
        Fail 5 "INTEGRATE BLOCKED: the checked-out branch changed since the run started (expected $Base); worktree kept at $Wt."
    }
    $Before = git rev-parse $Base
    git merge --ff-only --quiet "ralph/$RunId"
    if ($LASTEXITCODE -ne 0) {
        Fail 5 "INTEGRATE BLOCKED: fast-forward of $Base refused (uncommitted changes in the way?). recover with: git merge --ff-only ralph/$RunId && git worktree remove --force $Wt && git branch -D ralph/$RunId"
    }
    git worktree remove --force $Wt
    git branch --quiet -D "ralph/$RunId"
    Remove-OwnClaims

    Write-Host ""
    Write-Host "INTEGRATED into ${Base}:"
    git log --oneline "$Before..$Base" | ForEach-Object { Write-Host "  $_" }
}
finally {
    Remove-Item -LiteralPath $Lock -Recurse -Force -ErrorAction SilentlyContinue
}
