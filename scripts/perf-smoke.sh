#!/usr/bin/env bash
# M07 — Performance smoke: integration test, headless GPU (--dry-run), then --perf-smoke JSON.
#
# Usage:
#   ./scripts/perf-smoke.sh
#   ./scripts/perf-smoke.sh /path/to/editor-app
# Set PERF_SMOKE_STRICT=1 to enforce p99/max frame budgets (otherwise PERF_SMOKE_RELAX=1 is set).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "perf-smoke: cargo test smoke_test"
cargo test -p editor-app --test smoke_test --locked

BIN="${1:-$ROOT/target/debug/editor-app}"
if [[ ! -x "$BIN" ]] && [[ ! -f "$BIN" ]]; then
  cargo build -p editor-app --locked
  BIN="$ROOT/target/debug/editor-app"
fi

echo "perf-smoke: --dry-run ($BIN)"
"$BIN" --dry-run

if [[ "${PERF_SMOKE_STRICT:-}" != "1" ]]; then
  export PERF_SMOKE_RELAX=1
fi
echo "perf-smoke: --perf-smoke (PERF_SMOKE_STRICT=1 for strict frame budgets)"
exec "$BIN" --perf-smoke