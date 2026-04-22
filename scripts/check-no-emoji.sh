#!/usr/bin/env bash
# Fails when any Rust source file under `crates/` contains emoji / pictographic
# Unicode. Markdown + text files are allowed to carry emoji; this guard only
# protects code (where the UI intentionally uses rect-based icons instead).
#
# Usage:
#   ./scripts/check-no-emoji.sh           # scan + exit 0/1

set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
CRATES="$ROOT/crates"
if [[ ! -d "$CRATES" ]]; then
    echo "crates/ directory not found at $CRATES" >&2
    exit 2
fi

# Ranges (written as hex codepoint ranges for grep -P):
#   U+2600..U+27BF — Misc Symbols + Dingbats
#   U+1F300..U+1FAFF — every pictographic block we've seen in the wild
PATTERN='[\x{2600}-\x{27BF}\x{1F300}-\x{1F6FF}\x{1F700}-\x{1F77F}\x{1F900}-\x{1F9FF}\x{1FA70}-\x{1FAFF}]'

HITS=$(grep -rnP --include='*.rs' "$PATTERN" "$CRATES" | grep -vF 'allow-emoji' || true)

if [[ -n "$HITS" ]]; then
    echo "Emoji / pictographic characters found in Rust sources:" >&2
    echo "$HITS" >&2
    echo >&2
    echo "Use rect-based icons from editor-ui::icons instead." >&2
    exit 1
fi

exit 0
