#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd -P)"
cd "${ROOT}"

DIST="${ROOT}/dist"
TAURI_DIR="${TAURI_DIR:-${ROOT}/src-tauri}"
WEBUI_DIR="${WEBUI_DIR:-${ROOT}/webui}"
TAURI_TARGET_DIR="${TAURI_TARGET_DIR:-${TAURI_DIR}/target}"
TAURI_CLI="${TAURI_CLI:-${WEBUI_DIR}/node_modules/.bin/tauri}"

cargo_package_version() {
  cargo pkgid --package nexushubd --manifest-path "${ROOT}/Cargo.toml" |
    awk -F# '{print $NF}'
}

die() {
  echo "package-darwin-arm64.sh: $*" >&2
  exit 1
}

find_tauri_artifact() {
  local pattern="$1"
  local root
  local found=""

  for root in \
    "${TAURI_TARGET_DIR}/release/bundle" \
    "${TAURI_TARGET_DIR}/aarch64-apple-darwin/release/bundle"
  do
    [[ -d "${root}" ]] || continue
    found="$(find "${root}" -name "${pattern}" -print 2>/dev/null | sort | tail -n 1)"
    [[ -n "${found}" ]] && break
  done

  [[ -n "${found}" ]] || die "missing Tauri artifact matching ${pattern}"
  printf '%s\n' "${found}"
}

assert_app_only_archive() {
  local root="$1"
  local entries=()

  [[ -d "${root}/NexusHub.app" ]] || die "archive root must contain NexusHub.app"
  while IFS= read -r entry; do
    entries+=("${entry}")
  done < <(find "${root}" -mindepth 1 -maxdepth 1 -print | sort)

  if [[ "${#entries[@]}" -ne 1 || "${entries[0]}" != "${root}/NexusHub.app" ]]; then
    die "archive root must contain only NexusHub.app"
  fi
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

[[ -d "${TAURI_DIR}" ]] || die "missing Tauri project directory: ${TAURI_DIR}"
[[ -d "${WEBUI_DIR}" ]] || die "missing WebUI project directory: ${WEBUI_DIR}"

mkdir -p "${DIST}"

if [[ "${SKIP_WEBUI_INSTALL:-0}" != "1" ]]; then
  corepack pnpm@11.0.8 --dir "${WEBUI_DIR}" install
fi

if [[ "${SKIP_WEBUI_BUILD:-0}" != "1" ]]; then
  corepack pnpm@11.0.8 --dir "${WEBUI_DIR}" build:tauri
fi

[[ -x "${TAURI_CLI}" ]] || die "missing Tauri CLI: ${TAURI_CLI}"

if [[ "${SKIP_TAURI_BUILD:-0}" != "1" ]]; then
  "${TAURI_CLI}" build --config "${TAURI_DIR}/tauri.conf.json" --bundles app,dmg
fi

APP_BUNDLE="$(find_tauri_artifact "NexusHub.app")"
TAURI_DMG="$(find_tauri_artifact "*.dmg")"

TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT

APP_ARCHIVE_ROOT="${TMP}/app-archive"
mkdir -p "${APP_ARCHIVE_ROOT}"
cp -a "${APP_BUNDLE}" "${APP_ARCHIVE_ROOT}/NexusHub.app"

if [[ -n "${MACOS_CODESIGN_IDENTITY:-}" ]]; then
  codesign --force --deep --sign "${MACOS_CODESIGN_IDENTITY}" "${APP_ARCHIVE_ROOT}/NexusHub.app"
  codesign --verify --deep --strict "${APP_ARCHIVE_ROOT}/NexusHub.app"
fi

assert_app_only_archive "${APP_ARCHIVE_ROOT}"
tar -C "${APP_ARCHIVE_ROOT}" -czf "${DIST}/${TARBALL_ASSET}" NexusHub.app

cp "${TAURI_DMG}" "${DIST}/${DMG_ASSET}"

(
  cd "${DIST}"
  shasum -a 256 "${TARBALL_ASSET}" > "${TARBALL_ASSET}.sha256"
  shasum -a 256 "${DMG_ASSET}" > "${DMG_ASSET}.sha256"
)

echo "${DIST}/${TARBALL_ASSET}"
echo "${DIST}/${DMG_ASSET}"
