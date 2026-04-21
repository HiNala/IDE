#!/usr/bin/env bash
# M11: optional codesign + notarize + DMG. Without MACOS_CODESIGN_IDENTITY, produces an unsigned DMG only.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

APP="target/release/bundle/osx/IDE.app"
DMG="${1:-target/release/ide.dmg}"

if [[ ! -d "$APP" ]]; then
  echo "macos-package: missing $APP — run: cargo bundle --release -p editor-app --format osx" >&2
  exit 1
fi

xattr -cr "$APP" || true

IDENTITY="${MACOS_CODESIGN_IDENTITY:-}"

if [[ -n "$IDENTITY" ]]; then
  codesign --force --deep --options runtime --timestamp --sign "$IDENTITY" "$APP"
  codesign --verify --deep --strict --verbose=2 "$APP"
  ZIP="${APP}.zip"
  ditto -c -k --keepParent "$APP" "$ZIP"
  xcrun notarytool submit "$ZIP" \
    --keychain-profile "${NOTARY_PROFILE:-ide-notary}" \
    --wait \
    --timeout 45m
  xcrun stapler staple "$APP"
  rm -f "$ZIP"
else
  echo "macos-package: MACOS_CODESIGN_IDENTITY unset — unsigned .app (Gatekeeper may warn)"
fi

hdiutil create -volname "IDE" -srcfolder "$APP" -ov -format UDZO "$DMG"

if [[ -n "$IDENTITY" ]]; then
  xcrun stapler staple "$DMG" || true
fi

echo "macos-package: wrote $DMG"
