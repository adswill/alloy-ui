#!/usr/bin/env bash
# Builds the counter as an AppImage and a Flatpak bundle, runs the AppImage,
# installs the Flatpak for the user, checks its contents inside the sandbox and
# tries to render a frame from it.
# Usage: verify-linux-pkgs.sh <path-to-alloy>
set -euo pipefail
ALLOY=${1:?usage: verify-linux-pkgs.sh <alloy>}
OUT="$PWD/bundle-out"
mkdir -p "$OUT"
"$ALLOY" bundle -p alloy-ui-examples --bin counter --format appimage,flatpak --out "$OUT"
find "$OUT" -maxdepth 3 | sort | head -40
AI=$(ls "$OUT"/*.AppImage)
FP=$(ls "$OUT"/*.flatpak)
SUM=${GITHUB_STEP_SUMMARY:-/dev/null}

T=$(mktemp -d)
chmod +x "$AI"; (cd "$T" && "$AI" --appimage-extract >/dev/null)
"$T/squashfs-root/AppRun" --headless "$T/ai.png"
test "$(stat -c%s "$T/ai.png")" -gt 1000
file "$AI" | head -1
echo "AppImage runs: rendered $(stat -c%s "$T/ai.png") bytes"

flatpak install --user -y --noninteractive "$FP"
ID=$(flatpak list --user --app --columns=application | grep -i 'alloyui' | head -1)
echo "installed Flatpak app id: $ID"
flatpak info --user "$ID" | head -12
# The binary must be inside the sandbox at /app/bin.
flatpak run --command=ls "$ID" /app/bin
flatpak run --command=sh "$ID" -c 'test -x /app/bin/counter'

# Soft check: render a frame from inside the sandbox (needs a software GL/Vulkan
# adapter in the runtime). Reported, not required.
mkdir -p "$T/share"
if flatpak run --filesystem="$T/share" "$ID" --headless "$T/share/fp.png" 2>"$T/fp.err" \
   && [ "$(stat -c%s "$T/share/fp.png" 2>/dev/null || echo 0)" -gt 1000 ]; then
  FRAME="rendered $(stat -c%s "$T/share/fp.png") bytes"
else
  FRAME="did not render: $(tail -c 300 "$T/fp.err" | tr '\n' ' ')"
fi
echo "Flatpak headless frame: $FRAME"
flatpak uninstall --user -y --noninteractive "$ID"
if flatpak list --user --app --columns=application | grep -q "$ID"; then
  echo "::error::Flatpak still installed after uninstall"; exit 1
fi

{
  echo "### Linux AppImage and Flatpak (counter, real run)"; echo
  echo "| Artifact | Bytes | Result |"; echo "|---|---|---|"
  echo "| AppImage | $(stat -c%s "$AI") | extracted and rendered a frame |"
  echo "| Flatpak | $(stat -c%s "$FP") | installed, /app/bin/counter present, uninstalled |"
  echo
  echo "Flatpak headless frame: $FRAME"
} | tee -a "$SUM"
