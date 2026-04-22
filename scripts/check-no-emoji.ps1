# Fails when any Rust source file under `crates/` contains emoji or
# pictographic Unicode. Markdown + text files are allowed to carry emoji;
# this guard only protects code (the UI uses rect-based icons instead).
#
# Usage:
#   powershell -File scripts/check-no-emoji.ps1
#   powershell -File scripts/check-no-emoji.ps1 -Verbose

param(
    [switch]$Verbose
)

$ErrorActionPreference = 'Stop'

# Regex covering the common emoji ranges. Surrogate pairs are matched as pairs
# so Windows PowerShell's 16-bit chars work correctly.
$pattern = '[\u2600-\u27BF]|\uD83C[\uDD00-\uDFFF]|\uD83D[\uDC00-\uDFFF]|\uD83E[\uDD00-\uDFFF]'

$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$cratesDir = Join-Path $root 'crates'
if (-not (Test-Path $cratesDir)) {
    Write-Error "crates/ directory not found at $cratesDir"
    exit 2
}

$hits = New-Object System.Collections.Generic.List[string]
$rustFiles = Get-ChildItem -Path $cratesDir -Filter *.rs -Recurse -File
foreach ($file in $rustFiles) {
    $text = [System.IO.File]::ReadAllText($file.FullName)
    $lines = $text -split "`r?`n"
    for ($i = 0; $i -lt $lines.Length; $i++) {
        if ($lines[$i] -match $pattern) {
            # Explicit per-line opt-in for legitimate grapheme / emoji tests.
            if ($lines[$i] -match 'allow-emoji') { continue }
            $rel = $file.FullName.Substring($root.Path.Length + 1)
            $lineNum = $i + 1
            $hits.Add("$rel`:$lineNum")
        }
    }
}

if ($hits.Count -gt 0) {
    Write-Host "Emoji or pictographic characters found in Rust sources:" -ForegroundColor Red
    foreach ($h in $hits) { Write-Host "  $h" }
    Write-Host ""
    Write-Host "Use rect-based icons from editor-ui::icons instead." -ForegroundColor Yellow
    exit 1
}

if ($Verbose) {
    Write-Host "Scanned $($rustFiles.Count) Rust files - no emoji found." -ForegroundColor Green
}
exit 0
