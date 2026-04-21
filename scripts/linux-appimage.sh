#!/usr/bin/env bash
# M11: build a self-contained AppImage from `cargo build --release -p editor-app` output.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

APPDIR="target/release/AppDir"
BIN="target/release/editor-app"
ICON="crates/editor-app/assets/icons/icon-256.png"

if [[ ! -f "$BIN" ]]; then
  echo "linux-appimage: missing $BIN — run: cargo build --release -p editor-app" >&2
  exit 1
fi
if [[ ! -f "$ICON" ]]; then
  echo "linux-appimage: missing $ICON" >&2
  exit 1
fi

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" \
  "$APPDIR/usr/share/icons/hicolor/256x256/apps"

cp "$BIN" "$APPDIR/usr/bin/ide"
chmod +x "$APPDIR/usr/bin/ide"

cp "$ICON" "$APPDIR/usr/share/icons/hicolor/256x256/apps/ide.png"
cp "$ICON" "$APPDIR/ide.png"

cat >"$APPDIR/ide.desktop" <<EOF
[Desktop Entry]
Name=IDE
Exec=ide %U
Icon=ide
Type=Application
Categories=Development;TextEditor;
Terminal=false
EOF
cp "$APPDIR/ide.desktop" "$APPDIR/usr/share/applications/ide.desktop"

cat >"$APPDIR/AppRun" <<'APPRUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "${0}")")"
exec "$HERE/usr/bin/ide" "$@"
APPRUN
chmod +x "$APPDIR/AppRun"

OUT="${1:-target/release/ide-x86_64.AppImage}"
if ! command -v appimagetool >/dev/null 2>&1; then
  echo "linux-appimage: appimagetool not in PATH (install AppImageKit; see docs/RELEASING.md)" >&2
  exit 1
fi

ARCH=x86_64 appimagetool "$APPDIR" "$OUT"
echo "linux-appimage: wrote $OUT"
