#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd -P)"
cd "${ROOT}"

DIST="${ROOT}/dist"
TAURI_DIR="${TAURI_DIR:-${ROOT}/src-tauri}"
WEBUI_DIR="${WEBUI_DIR:-${ROOT}/webui}"
TAURI_TARGET_DIR="${TAURI_TARGET_DIR:-${TAURI_DIR}/target}"
TAURI_CLI="${TAURI_CLI:-${WEBUI_DIR}/node_modules/.bin/tauri}"
TMP=""
HELPER_RESOURCE=""
HELPER_RESOURCE_BACKUP=""
HELPER_RESOURCE_HAD_ORIGINAL=0

restore_helper_resource() {
  if [[ -n "${HELPER_RESOURCE}" && -n "${HELPER_RESOURCE_BACKUP}" && -f "${HELPER_RESOURCE_BACKUP}" ]]; then
    if [[ "${HELPER_RESOURCE_HAD_ORIGINAL}" == "1" ]]; then
      cp -p "${HELPER_RESOURCE_BACKUP}" "${HELPER_RESOURCE}"
    else
      rm -f "${HELPER_RESOURCE}"
    fi
  fi
}

cleanup() {
  [[ -n "${TMP}" && -d "${TMP}" ]] && rm -rf "${TMP}"
  restore_helper_resource
  [[ -n "${HELPER_RESOURCE_BACKUP}" && -f "${HELPER_RESOURCE_BACKUP}" ]] && rm -f "${HELPER_RESOURCE_BACKUP}"
}

trap cleanup EXIT

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

assert_app_bundle_resources() {
  local app_bundle="$1"
  local helper="${app_bundle}/Contents/Resources/nexushubd"
  local bundled_webui="${app_bundle}/Contents/Resources/webui"

  [[ -x "${helper}" ]] || die "app bundle missing executable nexushubd helper"
  local helper_kind
  helper_kind="$(file "${helper}")"
  [[ "${helper_kind}" == *"Mach-O 64-bit executable arm64"* ]] ||
    die "app bundle helper must be a macOS arm64 executable, got: ${helper_kind}"

  [[ -f "${bundled_webui}/index.html" ]] || die "app bundle missing WebUI index.html resource"
  [[ -d "${bundled_webui}/assets" ]] || die "app bundle missing WebUI assets resource"
  diff -qr "${WEBUI_DIR}/dist" "${bundled_webui}" >/dev/null ||
    die "app bundle WebUI resource does not match current webui/dist"
}

assert_helper_resource_placeholder() {
  [[ -f "${HELPER_RESOURCE}" ]] || die "helper placeholder missing after packaging cleanup"
  [[ ! -x "${HELPER_RESOURCE}" ]] || die "helper placeholder must remain non-executable after packaging cleanup"
  if ! grep -q '^NEXUSHUB_HELPER_PLACEHOLDER' "${HELPER_RESOURCE}"; then
    die "helper placeholder was not restored after packaging cleanup"
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
mkdir -p "${TAURI_DIR}/resources"
HELPER_RESOURCE="${TAURI_DIR}/resources/nexushubd"
HELPER_RESOURCE_BACKUP="$(mktemp)"
if [[ -f "${HELPER_RESOURCE}" ]]; then
  cp -p "${HELPER_RESOURCE}" "${HELPER_RESOURCE_BACKUP}"
  HELPER_RESOURCE_HAD_ORIGINAL=1
fi

if [[ "${SKIP_WEBUI_INSTALL:-0}" != "1" ]]; then
  corepack pnpm@11.0.8 --dir "${WEBUI_DIR}" install
fi

if [[ "${SKIP_WEBUI_BUILD:-0}" != "1" ]]; then
  corepack pnpm@11.0.8 --dir "${WEBUI_DIR}" build:tauri
fi

[[ -x "${TAURI_CLI}" ]] || die "missing Tauri CLI: ${TAURI_CLI}"

if [[ "${SKIP_HELPER_BUILD:-0}" != "1" ]]; then
  cargo build --release --package nexushubd
fi

HELPER_BINARY="${ROOT}/target/release/nexushubd"
[[ -x "${HELPER_BINARY}" ]] || die "missing helper binary: ${HELPER_BINARY}"
cp "${HELPER_BINARY}" "${HELPER_RESOURCE}"
chmod 755 "${HELPER_RESOURCE}"

if [[ "${SKIP_TAURI_BUILD:-0}" != "1" ]]; then
  "${TAURI_CLI}" build --config "${TAURI_DIR}/tauri.conf.json" --bundles app,dmg
fi

APP_BUNDLE="$(find_tauri_artifact "NexusHub.app")"
TAURI_DMG="$(find_tauri_artifact "*.dmg")"

assert_app_bundle_resources "${APP_BUNDLE}"

TMP="$(mktemp -d)"

APP_ARCHIVE_ROOT="${TMP}/app-archive"
mkdir -p "${APP_ARCHIVE_ROOT}"
cp -a "${APP_BUNDLE}" "${APP_ARCHIVE_ROOT}/NexusHub.app"
assert_app_bundle_resources "${APP_ARCHIVE_ROOT}/NexusHub.app"

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
cleanup
assert_helper_resource_placeholder
trap - EXIT
