#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd -P)"
DIST="${ROOT}/dist"
ASSET="nexushub-linux-x86_64.tar.gz"
OS="$(uname -s)"
ARCH="$(uname -m)"

if [[ "${ALLOW_HOST_MISMATCH:-0}" != "1" ]]; then
  if [[ "${OS}" != "Linux" || "${ARCH}" != "x86_64" ]]; then
    echo "package-linux.sh must run on Linux x86_64, got ${OS}/${ARCH}" >&2
    echo "Use GitHub Actions release workflow or set ALLOW_HOST_MISMATCH=1 only for local smoke archives." >&2
    exit 1
  fi
fi

mkdir -p "${DIST}"

if [[ "${SKIP_WEBUI_BUILD:-0}" != "1" ]]; then
  pnpm --dir "${ROOT}/webui" install
  VITE_BASE="${VITE_BASE:-/nexushub/}" pnpm --dir "${ROOT}/webui" build
fi

cargo build --release --package nexushubd

TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT
mkdir -p "${TMP}/nexushub/bin" "${TMP}/nexushub/webui" "${TMP}/nexushub/deploy"
cp "${ROOT}/target/release/nexushubd" "${TMP}/nexushub/bin/"
cp -a "${ROOT}/webui/dist/." "${TMP}/nexushub/webui/"
cp -a "${ROOT}/deploy/nexushub/." "${TMP}/nexushub/deploy/"
cp "${ROOT}/README.md" "${ROOT}/DESIGN.md" "${TMP}/nexushub/"

tar -C "${TMP}" -czf "${DIST}/${ASSET}" nexushub
(
  cd "${DIST}"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "${ASSET}" > "${ASSET}.sha256"
  else
    sha256sum "${ASSET}" > "${ASSET}.sha256"
  fi
)
echo "${DIST}/${ASSET}"
