# scripts/dev.ps1
#
# One-click dev launcher. Installs JS deps if missing, then runs the
# Tauri dev server which:
#
#   1. Starts Vite on http://localhost:5173 (hot-reload for the web side).
#   2. Compiles the Rust binary in dev mode and opens a Tauri window
#      pointed at the Vite server.
#
# Usage:  powershell -File scripts\dev.ps1
#
# Optional environment variables:
#   NEXUS_LIVE_TEST=1   — also run the live LM Studio tests on the way
#                         out (requires LM Studio to be reachable on
#                         http://127.0.0.1:1234/v1).

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Root      = Resolve-Path (Join-Path $ScriptDir "..")
Set-Location $Root

Write-Host "==> NEXUS dev launcher" -ForegroundColor Cyan
Write-Host "    root: $Root"

# Install JS deps if needed.
if (-not (Test-Path "app/web/node_modules")) {
    Write-Host "==> Installing web dependencies" -ForegroundColor Cyan
    Push-Location app/web
    try { npm install }
    finally { Pop-Location }
}

# Run Tauri dev.
Write-Host "==> Launching Tauri dev" -ForegroundColor Cyan
Push-Location app/src-tauri
try {
    if (Get-Command "cargo-tauri" -ErrorAction SilentlyContinue) {
        cargo tauri dev
    } else {
        Write-Host "    cargo-tauri not on PATH; using 'cargo tauri dev'" -ForegroundColor Yellow
        cargo tauri dev
    }
}
finally { Pop-Location }
