#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd -P)"
DIST="${ROOT}/dist"
ASSET="nexushub-webd-linux-x86_64.tar.gz"
ARCHIVE_ROOT="nexushub-webd-linux-x86_64"
OS="$(uname -s)"
ARCH="$(uname -m)"
CHECK_ONLY=0

for arg in "$@"; do
  case "${arg}" in
    --check) CHECK_ONLY=1 ;;
    *) echo "unknown argument: ${arg}" >&2; exit 2 ;;
  esac
done

die() { echo "package-webd-linux-x86_64.sh: $*" >&2; exit 1; }

if [[ "${ALLOW_HOST_MISMATCH:-0}" != "1" ]]; then
  if [[ "${OS}" != "Linux" || "${ARCH}" != "x86_64" ]]; then
    die "must run on Linux x86_64, got ${OS}/${ARCH}; use GitHub Actions or ALLOW_HOST_MISMATCH=1 for local smoke archives"
  fi
fi

mkdir -p "${DIST}"

if [[ "${SKIP_WEBUI_BUILD:-0}" != "1" ]]; then
  if [[ "${SKIP_WEBUI_INSTALL:-0}" != "1" ]]; then
    corepack pnpm@11.0.8 --dir "${ROOT}/webui" install
  fi
  VITE_BASE="${VITE_BASE:-/nexushub/}" \
    VITE_API_BASE="${VITE_API_BASE:-/nexushub}" \
    corepack pnpm@11.0.8 --dir "${ROOT}/webui" build
fi

cargo build --release --package nexushub-webd

TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT
mkdir -p "${TMP}/${ARCHIVE_ROOT}/bin" "${TMP}/${ARCHIVE_ROOT}/webui" "${TMP}/${ARCHIVE_ROOT}/deploy"
cp "${ROOT}/target/release/nexushub-webd" "${TMP}/${ARCHIVE_ROOT}/bin/"
cp -a "${ROOT}/webui/dist/." "${TMP}/${ARCHIVE_ROOT}/webui/"
cp -a "${ROOT}/deploy/nexushub-webd/." "${TMP}/${ARCHIVE_ROOT}/deploy/"
cp "${ROOT}/README.md" "${ROOT}/DESIGN.md" "${TMP}/${ARCHIVE_ROOT}/"
chmod 0755 "${TMP}/${ARCHIVE_ROOT}/bin/nexushub-webd"
chmod 0755 "${TMP}/${ARCHIVE_ROOT}/deploy/"*.sh
chmod 0755 "${TMP}/${ARCHIVE_ROOT}/deploy/nexushub-codex-"*

"${TMP}/${ARCHIVE_ROOT}/deploy/install.sh" --check
"${TMP}/${ARCHIVE_ROOT}/bin/nexushub-webd" --version | grep -Eq '^nexushub-webd [0-9]+\.[0-9]+\.[0-9]+'

tar -C "${TMP}" -czf "${DIST}/${ASSET}" "${ARCHIVE_ROOT}"
(
  cd "${DIST}"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "${ASSET}" > "${ASSET}.sha256"
  else
    sha256sum "${ASSET}" > "${ASSET}.sha256"
  fi
)

if [[ "${CHECK_ONLY}" -eq 1 ]]; then
  tar -tzf "${DIST}/${ASSET}" | grep -qx "${ARCHIVE_ROOT}/bin/nexushub-webd"
  tar -tzf "${DIST}/${ASSET}" | grep -qx "${ARCHIVE_ROOT}/webui/index.html"
  tar -tzf "${DIST}/${ASSET}" | grep -qx "${ARCHIVE_ROOT}/deploy/install.sh"
fi

echo "${DIST}/${ASSET}"
