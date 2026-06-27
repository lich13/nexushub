#!/usr/bin/env bash
set -Eeuo pipefail

REPO="${NEXUSHUB_WEBD_REPO:-lich13/nexushub}"
VERSION="latest"
ARCH="${NEXUSHUB_WEBD_ARCH:-x86_64}"
ASSET="nexushub-webd-linux-${ARCH}.tar.gz"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

usage() {
  cat <<'USAGE'
Update NexusHub WebUI daemon.

Usage:
  nexushub-webd-update --repo lich13/nexushub --version latest
  nexushub-webd-update v0.1.140
  nexushub-webd-update --precheck

Options:
  --repo REPO       GitHub repo. Default: lich13/nexushub
  --version TAG     Release tag or latest. Default: latest
  --arch ARCH       Release architecture suffix. Default: x86_64
  --precheck        Verify installed service and loopback health only.
  -h, --help        Show help.
USAGE
}

PRECHECK=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo) REPO="${2:-}"; shift 2 ;;
    --version) VERSION="${2:-}"; shift 2 ;;
    --arch) ARCH="${2:-}"; shift 2 ;;
    --precheck) PRECHECK=1; shift ;;
    -h|--help) usage; exit 0 ;;
    --*) echo "unknown argument: $1" >&2; exit 2 ;;
    *) VERSION="$1"; shift ;;
  esac
done

ASSET="nexushub-webd-linux-${ARCH}.tar.gz"

if [[ "${PRECHECK}" -eq 1 ]]; then
  test -x /usr/local/bin/nexushub-webd
  systemctl is-active --quiet nexushub-webd
  curl -fsS http://127.0.0.1:15742/healthz >/dev/null
  exit 0
fi

if [[ "$(id -u)" -ne 0 ]]; then
  echo "update.sh must run as root" >&2
  exit 1
fi

if [[ "${VERSION}" == "latest" ]]; then
  BASE_URL="https://github.com/${REPO}/releases/latest/download"
else
  RELEASE_TAG="${VERSION}"
  if [[ "${RELEASE_TAG}" != v* ]]; then
    RELEASE_TAG="v${RELEASE_TAG}"
  fi
  BASE_URL="https://github.com/${REPO}/releases/download/${RELEASE_TAG}"
fi

curl -fL "${BASE_URL}/${ASSET}" -o "${TMP_DIR}/${ASSET}"
curl -fL "${BASE_URL}/${ASSET}.sha256" -o "${TMP_DIR}/${ASSET}.sha256"
(cd "${TMP_DIR}" && sha256sum -c "${ASSET}.sha256")
tar -xzf "${TMP_DIR}/${ASSET}" -C "${TMP_DIR}"

ROOT="$(find "${TMP_DIR}" -maxdepth 1 -type d -name 'nexushub-webd-linux-*' | head -n 1)"
if [[ -z "${ROOT}" ]]; then
  echo "extracted archive root not found" >&2
  exit 1
fi

exec "${ROOT}/deploy/install.sh" --archive "${TMP_DIR}/${ASSET}"
