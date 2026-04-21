# M07 — Performance smoke: integration test, headless GPU, then `--perf-smoke` JSON (relaxed thresholds by default).
#
# Usage:
#   .\scripts\perf-smoke.ps1
#   .\scripts\perf-smoke.ps1 -Binary .\target\release\editor-app.exe
param(
    [string]$Binary = ""
)

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $RepoRoot

Write-Host "perf-smoke: cargo test smoke_test"
cargo test -p editor-app --test smoke_test --locked
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$exe = $Binary
if ([string]::IsNullOrWhiteSpace($exe)) {
    $exe = Join-Path $RepoRoot "target/debug/editor-app.exe"
    if (-not (Test-Path $exe)) {
        Write-Host "perf-smoke: building debug editor-app"
        cargo build -p editor-app --locked
    }
}
else {
    $exe = (Resolve-Path $exe).Path
    if (-not (Test-Path $exe)) {
        Write-Error "perf-smoke: binary not found: $exe"
        exit 2
    }
}

Write-Host "perf-smoke: --dry-run ($exe)"
& $exe --dry-run
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Scripted GPU sequence (~10 MiB buffer). Use strict budgets only when explicitly requested.
if ($env:PERF_SMOKE_STRICT -ne "1") {
    $env:PERF_SMOKE_RELAX = "1"
}
Write-Host "perf-smoke: --perf-smoke (set PERF_SMOKE_STRICT=1 to enforce p99<=16ms / max<=50ms)"
& $exe --perf-smoke
exit $LASTEXITCODE
