# Replace PATH agal.exe while MCP (agal serve) is running.
# Windows can rename a locked image; cargo install cannot overwrite it.
# Usage (from AGAL repo root):  powershell -File scripts/install-windows.ps1

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

cargo build --release
$src = Join-Path $root 'target\release\agal.exe'
$dstDir = Join-Path $env:USERPROFILE '.cargo\bin'
$dst = Join-Path $dstDir 'agal.exe'

if (Test-Path $dst) {
    $stamp = Get-Date -Format 'yyyyMMddHHmmss'
    $old = Join-Path $dstDir ("agal.exe.old." + $stamp)
    Rename-Item -LiteralPath $dst -NewName (Split-Path $old -Leaf) -Force
}

Copy-Item -Force $src $dst

# Drop stale .old copies that are no longer mapped.
Get-ChildItem -LiteralPath $dstDir -Filter 'agal.exe.old*' -ErrorAction SilentlyContinue |
    ForEach-Object {
        Remove-Item -LiteralPath $_.FullName -Force -ErrorAction SilentlyContinue
    }

& $dst --version
Write-Host "installed $dst"
Write-Host "reconnect MCP if serve is still on the old PID"
