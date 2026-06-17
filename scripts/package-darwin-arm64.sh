#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd -P)"
DIST="${ROOT}/dist"

cargo_package_version() {
  cargo pkgid --package nexushubd --manifest-path "${ROOT}/Cargo.toml" |
    awk -F# '{print $NF}'
}

OS="$(uname -s)"
ARCH="$(uname -m)"

if [[ "${ALLOW_HOST_MISMATCH:-0}" != "1" ]]; then
  if [[ "${OS}" != "Darwin" || "${ARCH}" != "arm64" ]]; then
    echo "package-darwin-arm64.sh must run on macOS arm64, got ${OS}/${ARCH}" >&2
    echo "Use GitHub Actions release workflow or set ALLOW_HOST_MISMATCH=1 only for local smoke archives." >&2
    exit 1
  fi
fi

VERSION="${VERSION:-$(cargo_package_version)}"
TARBALL_ASSET="nexushub-darwin-arm64.tar.gz"
DMG_ASSET="NexusHub-${VERSION}-darwin-arm64.dmg"

mkdir -p "${DIST}"

if [[ "${SKIP_WEBUI_BUILD:-0}" != "1" ]]; then
  corepack pnpm@11.0.8 --dir "${ROOT}/webui" install
  VITE_BASE="${VITE_BASE:-/nexushub/}" corepack pnpm@11.0.8 --dir "${ROOT}/webui" build
fi

cargo build --release --package nexushubd

TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT

PAYLOAD="${TMP}/NexusHub"
mkdir -p "${PAYLOAD}/bin" "${PAYLOAD}/webui/dist"
cp "${ROOT}/target/release/nexushubd" "${PAYLOAD}/bin/"
cp -a "${ROOT}/webui/dist/." "${PAYLOAD}/webui/dist/"
cp "${ROOT}/deploy/nexushub/macos/install.sh" "${PAYLOAD}/"
cp "${ROOT}/deploy/nexushub/macos/uninstall.sh" "${PAYLOAD}/"
cp "${ROOT}/deploy/nexushub/macos/com.nexushub.nexushub.plist" "${PAYLOAD}/"
cp "${ROOT}/deploy/nexushub/macos/README.md" "${PAYLOAD}/"
chmod +x "${PAYLOAD}/install.sh" "${PAYLOAD}/uninstall.sh" "${PAYLOAD}/bin/nexushubd"

tar -C "${TMP}" -czf "${DIST}/${TARBALL_ASSET}" NexusHub

DMG_SRC="${TMP}/dmg"
mkdir -p "${DMG_SRC}"
cp -a "${PAYLOAD}" "${DMG_SRC}/NexusHub"
cat > "${DMG_SRC}/Install.command" <<'COMMAND'
#!/usr/bin/env bash
set -Eeuo pipefail
DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd -P)"
exec "${DIR}/NexusHub/install.sh"
COMMAND
cat > "${DMG_SRC}/Uninstall.command" <<'COMMAND'
#!/usr/bin/env bash
set -Eeuo pipefail
DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd -P)"
exec "${DIR}/NexusHub/uninstall.sh"
COMMAND
chmod +x "${DMG_SRC}/Install.command" "${DMG_SRC}/Uninstall.command"

APP_DIR="${DMG_SRC}/NexusHub.app"
mkdir -p "${APP_DIR}/Contents/MacOS" "${APP_DIR}/Contents/Resources"
cat > "${APP_DIR}/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>NexusHub</string>
  <key>CFBundleIdentifier</key>
  <string>com.nexushub.launcher</string>
  <key>CFBundleName</key>
  <string>NexusHub</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>__VERSION__</string>
  <key>CFBundleVersion</key>
  <string>__VERSION__</string>
  <key>LSMinimumSystemVersion</key>
  <string>14.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST
sed -i '' "s#__VERSION__#${VERSION}#g" "${APP_DIR}/Contents/Info.plist"
cat > "${APP_DIR}/Contents/MacOS/NexusHub" <<'APP'
#!/usr/bin/env bash
open "http://127.0.0.1:15742/nexushub/"
APP
chmod +x "${APP_DIR}/Contents/MacOS/NexusHub"
ln -s /Applications "${DMG_SRC}/Applications"

hdiutil create \
  -volname "NexusHub ${VERSION}" \
  -srcfolder "${DMG_SRC}" \
  -ov \
  -format UDZO \
  "${DIST}/${DMG_ASSET}"

if [[ -n "${MACOS_CODESIGN_IDENTITY:-}" ]]; then
  codesign --force --sign "${MACOS_CODESIGN_IDENTITY}" "${DIST}/${DMG_ASSET}"
fi

(
  cd "${DIST}"
  shasum -a 256 "${TARBALL_ASSET}" > "${TARBALL_ASSET}.sha256"
  shasum -a 256 "${DMG_ASSET}" > "${DMG_ASSET}.sha256"
)

echo "${DIST}/${TARBALL_ASSET}"
echo "${DIST}/${DMG_ASSET}"
