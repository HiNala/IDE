# M12 — Resize stress helper (Windows).
# Runs the editor with resize telemetry. Manually drag window edges while it runs.
param(
    [string]$EditorExe = ".\target\release\editor-app.exe"
)

if (-not (Test-Path $EditorExe)) {
    Write-Host "resize-stress: $EditorExe not found. Run: cargo build --release -p editor-app"
    exit 1
}

$env:RUST_LOG = if ($env:RUST_LOG) { $env:RUST_LOG } else { "editor_app::resize_telemetry=info" }
& $EditorExe --resize-telemetry @args
