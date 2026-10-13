# scripts/check.ps1
#
# One-shot CI check. Runs the full test matrix in this order:
#
#   1. Vite production build  (TypeScript + bundle).
#   2. Cargo test (lib + tests)  (Rust unit + integration + live).
#   3. Cargo check --release     (release-mode compile sanity).
#
# The script returns the first non-zero exit code, so it can be wired
# straight into CI.
#
# Usage:  powershell -File scripts\check.ps1
#
# Environment variables:
#   NEXUS_SKIP_WEB=1      — skip the Vite build.
#   NEXUS_SKIP_RELEASE=1  — skip `cargo check --release`.

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Root      = Resolve-Path (Join-Path $ScriptDir "..")
Set-Location $Root

$skipWeb     = $env:NEXUS_SKIP_WEB     -eq "1"
$skipRelease = $env:NEXUS_SKIP_RELEASE -eq "1"
$failed      = 0

function Step($name, $block) {
    Write-Host ""
    Write-Host "==> $name" -ForegroundColor Cyan
    & $block
    if ($LASTEXITCODE -ne 0) {
        Write-Host "    FAILED (exit $LASTEXITCODE)" -ForegroundColor Red
        $script:failed = $LASTEXITCODE
    } else {
        Write-Host "    ok" -ForegroundColor Green
    }
}

if (-not $skipWeb) {
    Step "Vite production build" {
        Push-Location app/web
        try { npm run build }
        finally { Pop-Location }
    }
}

Step "Cargo test (lib + tests)" {
    Push-Location app/src-tauri
    try { cargo test --lib --tests }
    finally { Pop-Location }
}

if (-not $skipRelease) {
    Step "Cargo check --release" {
        Push-Location app/src-tauri
        try { cargo check --release }
        finally { Pop-Location }
    }
}

Write-Host ""
if ($failed -eq 0) {
    Write-Host "==> All checks green." -ForegroundColor Green
    exit 0
} else {
    Write-Host "==> One or more checks failed ($failed)." -ForegroundColor Red
    exit $failed
}
