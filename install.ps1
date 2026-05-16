#Requires -Version 5
# Install script for homeos
# Usage: irm https://raw.githubusercontent.com/hainet50b/homeos/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "hainet50b/homeos"
$InstallDir = if ($env:HOMEOS_INSTALL_DIR) { $env:HOMEOS_INSTALL_DIR } else { "$env:USERPROFILE\.homeos\bin" }

$Arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
$Target = switch ($Arch) {
    "X64"   { "x86_64-pc-windows-msvc" }
    "Arm64" { "aarch64-pc-windows-msvc" }
    default { throw "Unsupported architecture: $Arch" }
}

$Url = "https://github.com/$Repo/releases/latest/download/homeos-$Target.zip"

$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) "homeos-install-$([guid]::NewGuid())"
New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
$TempZip = Join-Path $TempDir "homeos.zip"

try {
    Write-Host "Downloading homeos for $Target..."
    Invoke-WebRequest -Uri $Url -OutFile $TempZip -UseBasicParsing

    Write-Host "Extracting..."
    Expand-Archive -Path $TempZip -DestinationPath $TempDir -Force

    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    }
    Move-Item -Path (Join-Path $TempDir "homeos.exe") -Destination (Join-Path $InstallDir "homeos.exe") -Force

    Write-Host "Installed homeos to $InstallDir\homeos.exe"

    $CompletionDir = Join-Path $env:USERPROFILE ".homeos"
    $CompletionFile = Join-Path $CompletionDir "completion.ps1"
    if (-not (Test-Path $CompletionDir)) {
        New-Item -ItemType Directory -Force -Path $CompletionDir | Out-Null
    }
    & (Join-Path $InstallDir "homeos.exe") completion powershell | Out-File -FilePath $CompletionFile -Encoding utf8
    Write-Host ""
    Write-Host "Installed PowerShell completion to $CompletionFile"
    Write-Host "Add the following line to your `$PROFILE to enable completion:"
    Write-Host ""
    Write-Host "    . `"$CompletionFile`""
    Write-Host ""

    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $PathParts = if ($UserPath) { $UserPath -split ';' | Where-Object { $_ -ne '' } } else { @() }
    if ($PathParts -notcontains $InstallDir) {
        $NewPath = ((@($PathParts) + $InstallDir) -join ';')
        [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
        Write-Host ""
        Write-Host "Added $InstallDir to your user PATH."
        Write-Host "Open a new terminal to use 'homeos'."
    } else {
        Write-Host ""
        & (Join-Path $InstallDir "homeos.exe") --version
    }
} finally {
    if (Test-Path $TempDir) { Remove-Item $TempDir -Recurse -Force }
}
