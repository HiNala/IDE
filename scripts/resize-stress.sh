#!/usr/bin/env bash
# M12 — Resize stress helper (Unix). Runs the editor with resize telemetry.
# Manually drag window edges while it runs, or rely on CI gpu_resize_stress tests.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
EXE="${EDITOR_EXE:-$ROOT/target/release/editor-app}"

if [[ ! -x "$EXE" ]] && [[ ! -f "$EXE" ]]; then
  echo "resize-stress: $EXE not found. Run: cargo build --release -p editor-app" >&2
  exit 1
fi

export RUST_LOG="${RUST_LOG:-editor_app::resize_telemetry=info}"
exec "$EXE" --resize-telemetry "$@"
