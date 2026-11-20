#!/usr/bin/env bash
# Bundles the counter example as deb, rpm and AppImage and checks each.
# Usage: verify-linux.sh <path-to-alloy>
set -euo pipefail
ALLOY=${1:?usage: verify-linux.sh <alloy>}
OUT="$PWD/bundle-out"
rm -rf "$OUT"
"$ALLOY" bundle -p alloy-ui-examples --bin counter --format deb,rpm,appimage --out "$OUT"
ls -l "$OUT"
DEB=$(ls "$OUT"/*.deb); RPM=$(ls "$OUT"/*.rpm); AI=$(ls "$OUT"/*.AppImage)
dpkg-deb -I "$DEB" | head -12
dpkg-deb -c "$DEB" | grep -E 'usr/bin/counter|\.desktop|256x256'
rpm -qp --info "$RPM" | head -5
rpm -K "$RPM"
# Extract the deb and render a frame with the packaged binary.
X=$(mktemp -d); dpkg-deb -x "$DEB" "$X"
"$X/usr/bin/counter" --headless "$X/f.png"; test "$(stat -c%s "$X/f.png")" -gt 1000
# AppImage: extract (no FUSE on runners) and run.
chmod +x "$AI"; (cd "$X" && "$AI" --appimage-extract >/dev/null)
"$X/squashfs-root/AppRun" --headless "$X/g.png"; test "$(stat -c%s "$X/g.png")" -gt 1000
{
  echo "### Linux bundle sizes (counter, real run)"; echo
  echo "| Artifact | Bytes |"; echo "|---|---|"
  echo "| counter binary | $(stat -c%s "$X/usr/bin/counter") |"
  echo "| .deb | $(stat -c%s "$DEB") |"
  echo "| .rpm | $(stat -c%s "$RPM") |"
  echo "| AppImage | $(stat -c%s "$AI") |"
} | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
