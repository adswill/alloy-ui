#!/usr/bin/env bash
# Bundles the counter example as .app and .dmg and proves them on a real runner:
# signature verifies, plist lints, the binary renders a frame, the dmg mounts.
# Usage: verify-macos.sh <path-to-alloy>
set -euo pipefail
ALLOY=${1:?usage: verify-macos.sh <alloy>}
OUT="$PWD/bundle-out"
rm -rf "$OUT"
"$ALLOY" bundle -p alloy-ui-examples --bin counter --format app,dmg --out "$OUT"
find "$OUT" -maxdepth 3 | sort

APP=$(find "$OUT" -maxdepth 2 -name '*.app' | head -1)
DMG=$(find "$OUT" -maxdepth 2 -name '*.dmg' | head -1)
[ -d "$APP" ] && [ -f "$DMG" ] || { echo "::error::missing .app or .dmg"; exit 1; }

plutil -lint "$APP/Contents/Info.plist"
test -f "$APP/Contents/Resources/icon.icns"
file "$APP/Contents/Resources/icon.icns" | grep -qi 'icon' || { echo "::error::icon.icns is not an icon"; exit 1; }
codesign --verify --deep --strict --verbose=2 "$APP"
codesign -dv "$APP" 2>&1 | grep -E 'Identifier|Signature|Format'

check_run() {
  local png; png=$(mktemp -t counter).png
  "$1" --headless "$png"
  [ "$(stat -f%z "$png")" -gt 1000 ] || { echo "::error::$1 rendered no frame"; exit 1; }
  echo "$1 rendered $(stat -f%z "$png") bytes"
}
check_run "$APP/Contents/MacOS/counter"

# Mount the dmg and run the copy inside it, as a user would after dragging it out.
MNT=$(mktemp -d)
hdiutil attach "$DMG" -mountpoint "$MNT" -nobrowse -readonly -quiet
trap 'hdiutil detach "$MNT" -quiet || true' EXIT
INAPP=$(find "$MNT" -maxdepth 1 -name '*.app' | head -1)
[ -d "$INAPP" ] || { echo "::error::no .app inside the dmg"; exit 1; }
test -L "$MNT/Applications"
codesign --verify --deep --strict "$INAPP"
check_run "$INAPP/Contents/MacOS/counter"

size_dir() { du -sk "$1" | cut -f1; }
{
  echo "### macOS bundle sizes (counter, real run)"; echo
  echo "| Artifact | Bytes |"; echo "|---|---|"
  echo "| counter binary | $(stat -f%z "$APP/Contents/MacOS/counter") |"
  echo "| .app (du, KiB) | $(size_dir "$APP") KiB |"
  echo "| .dmg | $(stat -f%z "$DMG") |"
} | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
