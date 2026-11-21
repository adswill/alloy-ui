#!/usr/bin/env bash
# Developer ID signing and Apple notarisation of the counter. Only runs when the
# repository has the secrets below; see docs/guide/17-shipping.md.
#
#   MACOS_CERT_P12_BASE64   base64 of a "Developer ID Application" .p12
#   MACOS_CERT_PASSWORD     its password
#   MACOS_SIGN_IDENTITY     e.g. "Developer ID Application: Example Inc. (ABCDE12345)"
#   NOTARY_APPLE_ID, NOTARY_TEAM_ID, NOTARY_PASSWORD   (app-specific password)
#
# Usage: notarize-macos.sh <path-to-alloy>
set -euo pipefail
ALLOY=${1:?usage: notarize-macos.sh <alloy>}
KC="$RUNNER_TEMP/alloy-ci.keychain-db"
KCPW=$(uuidgen)
security create-keychain -p "$KCPW" "$KC"
security set-keychain-settings -lut 21600 "$KC"
security unlock-keychain -p "$KCPW" "$KC"
echo "$MACOS_CERT_P12_BASE64" | base64 --decode > "$RUNNER_TEMP/cert.p12"
security import "$RUNNER_TEMP/cert.p12" -k "$KC" -P "$MACOS_CERT_PASSWORD" -T /usr/bin/codesign
security set-key-partition-list -S apple-tool:,apple: -s -k "$KCPW" "$KC" >/dev/null
security list-keychains -d user -s "$KC" login.keychain

export ALLOY_SIGN_IDENTITY="$MACOS_SIGN_IDENTITY"
export ALLOY_NOTARY_APPLE_ID="$NOTARY_APPLE_ID"
export ALLOY_NOTARY_TEAM_ID="$NOTARY_TEAM_ID"
export ALLOY_NOTARY_PASSWORD="$NOTARY_PASSWORD"
OUT="$PWD/notarized-out"
mkdir -p "$OUT"
"$ALLOY" bundle -p alloy-ui-examples --bin counter --format app,dmg --out "$OUT"
APP=$(find "$OUT" -maxdepth 2 -name '*.app' | head -1)
DMG=$(find "$OUT" -maxdepth 2 -name '*.dmg' | head -1)
codesign --verify --deep --strict --verbose=2 "$APP"
xcrun stapler validate "$APP"
spctl --assess --type execute --verbose=4 "$APP"
{
  echo "### macOS Developer ID signing and notarisation"; echo
  echo "Signed, notarised and stapled; Gatekeeper (spctl) accepts \`$(basename "$APP")\`. dmg: $(stat -f%z "$DMG") bytes."
} | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
