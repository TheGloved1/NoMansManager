#!/usr/bin/env bash
set -euo pipefail

PKG_VER="$(bun -e "console.log(require('./package.json').version)")"
PLATFORM="${PLATFORM:-windows-x86_64}"
echo "Generating updater fragment for $PLATFORM version $PKG_VER"

case "$PLATFORM" in
  windows-x86_64)
    CANDIDATES=(src-tauri/target/release/bundle/msi/NoMansManager.msi)
    PLATFORM_KEY="windows-x86_64"
    ;;
  linux-x86_64)
    CANDIDATES=(src-tauri/target/release/bundle/appimage/NoMansManager.AppImage)
    PLATFORM_KEY="linux-x86_64"
    ;;
  darwin-aarch64)
    # NOTE: the updater requires the .tar.gz — never the .dmg.
    CANDIDATES=(src-tauri/target/release/bundle/macos/NoMansManager.tar.gz)
    PLATFORM_KEY="darwin-aarch64"
    ;;
  *)
    echo "Error: Unknown platform $PLATFORM"
    exit 1
    ;;
esac

BUNDLE_FILE=""
for c in "${CANDIDATES[@]}"; do
  if [ -f "$c" ]; then BUNDLE_FILE="$c"; break; fi
done

if [ -z "$BUNDLE_FILE" ]; then
  echo "Error: No bundle file found for platform $PLATFORM. Bundle tree:" >&2
  find src-tauri/target/release/bundle -maxdepth 2 2>/dev/null | sort >&2 || true
  exit 1
fi
echo "Bundle: $BUNDLE_FILE"

SIGNATURE=""
if [ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
  echo "Attempting signature generation..."
  SIGNER_OUTPUT=$(SHELL=/bin/bash bun tauri signer sign \
    -k "$TAURI_SIGNING_PRIVATE_KEY" \
    -p "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" \
    "$BUNDLE_FILE" 2>/dev/null || true)
  SIGNATURE=$(echo "$SIGNER_OUTPUT" | awk '/^Public signature:/{getline; print}')
  if [ -n "$SIGNATURE" ]; then
    echo "Signature generated successfully (${#SIGNATURE} chars)"
  fi
fi

if [ -z "$SIGNATURE" ]; then
  echo "Warning: No signature generated — updater will reject unsigned updates"
fi

BUNDLE_BASENAME=$(basename "$BUNDLE_FILE")
TAG_NAME="v${PKG_VER}"
DOWNLOAD_URL="https://github.com/TheGloved1/NoMansManager/releases/download/${TAG_NAME}/${BUNDLE_BASENAME}"
PUB_DATE=$(date -u +"%Y-%m-%dT%H:%M:%S.000Z")

cat > "updater-${PLATFORM}.json" <<EOF
{
  "version": "${PKG_VER}",
  "notes": "",
  "pub_date": "${PUB_DATE}",
  "platforms": {
    "${PLATFORM_KEY}": {
      "signature": "${SIGNATURE}",
      "url": "${DOWNLOAD_URL}"
    }
  }
}
EOF

echo "updater-${PLATFORM}.json generated successfully"
