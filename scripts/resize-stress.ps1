# M12 — Resize stress helper (Windows).
# Builds nothing; runs a release binary with resize telemetry enabled.
# Manually drag the window edges while this process runs, then inspect logs.
param(
    [string]$EditorExe = ".\target\release\editor-app.exe"
)

if (-not (Test-Path $EditorExe)) {
    Write-Host "resize-stress: $EditorExe not found. Run: cargo build --release -p editor-app"
    exit 1
}

$env:RUST_LOG = "editor_app::resize_telemetry=info"
& $EditorExe --resize-telemetry @args
