# scripts/release.ps1
#
# Build the Windows installer. The output lands in
#   app/src-tauri/target/release/bundle/{nsis,msi}/
#
# Usage:  powershell -File scripts\release.ps1
#
# Environment variables:
#   NEXUS_SKIP_WEB=1   — skip the Vite build (assumes dist/ is current).
#   NEXUS_NO_BUNDLE=1  — only build the .exe (no NSIS / MSI).

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Root      = Resolve-Path (Join-Path $ScriptDir "..")
Set-Location $Root

$skipWeb    = $env:NEXUS_SKIP_WEB    -eq "1"
$noBundle   = $env:NEXUS_NO_BUNDLE   -eq "1"

if (-not $skipWeb) {
    Write-Host "==> Vite production build" -ForegroundColor Cyan
    Push-Location app/web
    try { npm run build }
    finally { Pop-Location }
    if ($LASTEXITCODE -ne 0) { throw "vite build failed" }
}

Write-Host "==> cargo tauri build" -ForegroundColor Cyan
Push-Location app/src-tauri
try {
    $args = @("tauri", "build")
    if ($noBundle) { $args += "--no-bundle" }
    & cargo @args
}
finally { Pop-Location }

if ($LASTEXITCODE -ne 0) { throw "tauri build failed" }

Write-Host ""
Write-Host "==> Done. Bundles:" -ForegroundColor Green
if (Test-Path "app/src-tauri/target/release/bundle/nsis") {
    Get-ChildItem "app/src-tauri/target/release/bundle/nsis" -File
}
if (Test-Path "app/src-tauri/target/release/bundle/msi") {
    Get-ChildItem "app/src-tauri/target/release/bundle/msi" -File
}
if (Test-Path "app/src-tauri/target/release/nexus.exe") {
    Get-Item "app/src-tauri/target/release/nexus.exe"
}
