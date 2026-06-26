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
UNSIGNED_TAURI_CONFIG=""

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
  [[ -n "${UNSIGNED_TAURI_CONFIG}" && -f "${UNSIGNED_TAURI_CONFIG}" ]] && rm -f "${UNSIGNED_TAURI_CONFIG}"
  return 0
}

trap cleanup EXIT

cargo_package_version() {
  cargo pkgid --package nexushubd --manifest-path "${ROOT}/Cargo.toml" |
    awk -F# '{print $NF}'
}

die() {
  echo "package-linux-tauri-x86_64.sh: $*" >&2
  exit 1
}

find_tauri_artifact() {
  local pattern="$1"
  local root
  local found=""

  for root in \
    "${TAURI_TARGET_DIR}/release/bundle" \
    "${TAURI_TARGET_DIR}/x86_64-unknown-linux-gnu/release/bundle"
  do
    [[ -d "${root}" ]] || continue
    found="$(find "${root}" -name "${pattern}" -print 2>/dev/null | sort | tail -n 1)"
    [[ -n "${found}" ]] && break
  done

  [[ -n "${found}" ]] || die "missing Tauri artifact matching ${pattern}"
  printf '%s\n' "${found}"
}

assert_desktop_binary() {
  local binary="${TAURI_TARGET_DIR}/release/nexushub"
  local helper="${TAURI_TARGET_DIR}/release/nexushubd"
  [[ -x "${binary}" ]] || die "desktop build did not produce executable ${binary}"
  if [[ -x "${helper}" ]] && cmp -s "${binary}" "${helper}"; then
    die "desktop binary unexpectedly matches server helper at ${helper}"
  fi
}

assert_bundle_resources() {
  local bundle_root="${TAURI_TARGET_DIR}/release/bundle"
  local helper_count
  local webui_count

  [[ -d "${bundle_root}" ]] || die "missing Linux Tauri bundle directory: ${bundle_root}"
  helper_count="$(find "${bundle_root}" -type f -name "nexushubd" -print 2>/dev/null | wc -l | tr -d ' ')"
  webui_count="$(find "${bundle_root}" -type f -path "*/webui/index.html" -print 2>/dev/null | wc -l | tr -d ' ')"
  [[ "${helper_count}" -gt 0 ]] || die "Linux Tauri bundle missing nexushubd resource"
  [[ "${webui_count}" -gt 0 ]] || die "Linux Tauri bundle missing webui/index.html resource"
}

assert_helper_resource_placeholder() {
  [[ -f "${HELPER_RESOURCE}" ]] || die "helper placeholder missing after packaging cleanup"
  [[ ! -x "${HELPER_RESOURCE}" ]] || die "helper placeholder must remain non-executable after packaging cleanup"
  if ! grep -q '^NEXUSHUB_HELPER_PLACEHOLDER' "${HELPER_RESOURCE}"; then
    die "helper placeholder was not restored after packaging cleanup"
  fi
}

write_sha256() {
  local asset="$1"
  (
    cd "${DIST}"
    if command -v sha256sum >/dev/null 2>&1; then
      sha256sum "${asset}" > "${asset}.sha256"
    else
      shasum -a 256 "${asset}" > "${asset}.sha256"
    fi
  )
}

copy_asset_with_sha256() {
  local source="$1"
  local asset="$2"
  cp "${source}" "${DIST}/${asset}"
  write_sha256 "${asset}"
}

OS="$(uname -s)"
ARCH="$(uname -m)"

if [[ "${ALLOW_HOST_MISMATCH:-0}" != "1" ]]; then
  if [[ "${OS}" != "Linux" || "${ARCH}" != "x86_64" ]]; then
    echo "package-linux-tauri-x86_64.sh must run on Linux x86_64, got ${OS}/${ARCH}" >&2
    echo "Use GitHub Actions release workflow or set ALLOW_HOST_MISMATCH=1 only for local smoke archives." >&2
    exit 1
  fi
fi

VERSION="${VERSION:-$(cargo_package_version)}"
APPIMAGE_ASSET="NexusHub-${VERSION}-Linux-x86_64.AppImage"
DEB_ASSET="NexusHub-${VERSION}-Linux-x86_64.deb"
RPM_ASSET="NexusHub-${VERSION}-Linux-x86_64.rpm"
SIGNED_RELEASE_CONTEXT=0
if [[ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" || -n "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]]; then
  SIGNED_RELEASE_CONTEXT=1
elif [[ "${GITHUB_ACTIONS:-}" == "true" && "${ALLOW_UNSIGNED_TAURI_UPDATER:-0}" != "1" ]]; then
  SIGNED_RELEASE_CONTEXT=1
fi
if [[ "${SIGNED_RELEASE_CONTEXT}" == "1" && -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
  die "release builds require TAURI_SIGNING_PRIVATE_KEY for signed Linux updater artifacts"
fi

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

TAURI_BUILD_CONFIG="${TAURI_DIR}/tauri.conf.json"
if [[ "${SIGNED_RELEASE_CONTEXT}" != "1" ]]; then
  UNSIGNED_TAURI_CONFIG="$(mktemp)"
  python3 - "${TAURI_DIR}/tauri.conf.json" "${UNSIGNED_TAURI_CONFIG}" <<'PY'
import json
import sys

source, target = sys.argv[1:]
with open(source, "r", encoding="utf-8") as fh:
    config = json.load(fh)
config.setdefault("bundle", {})["createUpdaterArtifacts"] = False
config.get("plugins", {}).pop("updater", None)
with open(target, "w", encoding="utf-8") as fh:
    json.dump(config, fh, ensure_ascii=False, indent=2)
    fh.write("\n")
PY
  TAURI_BUILD_CONFIG="${UNSIGNED_TAURI_CONFIG}"
fi

if [[ "${SKIP_TAURI_BUILD:-0}" != "1" ]]; then
  "${TAURI_CLI}" build --config "${TAURI_BUILD_CONFIG}" --bundles appimage,deb,rpm
fi

assert_desktop_binary
assert_bundle_resources

TAURI_APPIMAGE="$(find_tauri_artifact "*.AppImage")"
TAURI_DEB="$(find_tauri_artifact "*.deb")"
TAURI_RPM="$(find_tauri_artifact "*.rpm")"

copy_asset_with_sha256 "${TAURI_APPIMAGE}" "${APPIMAGE_ASSET}"
copy_asset_with_sha256 "${TAURI_DEB}" "${DEB_ASSET}"
copy_asset_with_sha256 "${TAURI_RPM}" "${RPM_ASSET}"

if [[ -f "${TAURI_APPIMAGE}.sig" ]]; then
  cp "${TAURI_APPIMAGE}.sig" "${DIST}/${APPIMAGE_ASSET}.sig"
elif [[ "${SIGNED_RELEASE_CONTEXT}" == "1" ]]; then
  die "missing Linux AppImage updater signature; set TAURI_SIGNING_PRIVATE_KEY for release builds"
fi

echo "${DIST}/${APPIMAGE_ASSET}"
echo "${DIST}/${DEB_ASSET}"
echo "${DIST}/${RPM_ASSET}"
cleanup
assert_helper_resource_placeholder
trap - EXIT
