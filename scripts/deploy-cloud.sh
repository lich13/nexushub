#!/usr/bin/env bash
set -Eeuo pipefail

HOST="${1:?Usage: deploy-cloud.sh HOST [ARCHIVE]; set NEXUSHUB_DOMAIN explicitly}"
ARCHIVE="${2:-dist/nexushub-webd-linux-x86_64.tar.gz}"
DOMAIN="${NEXUSHUB_DOMAIN:?Set NEXUSHUB_DOMAIN explicitly}"
PATH_PREFIX="${NEXUSHUB_PATH_PREFIX:-/nexushub/}"
EXPECTED_VERSION="${NEXUSHUB_EXPECTED_VERSION:-}"

if [[ ! -f "${ARCHIVE}" ]]; then
  echo "archive not found: ${ARCHIVE}" >&2
  exit 1
fi

if [[ -z "${EXPECTED_VERSION}" ]]; then
  EXPECTED_VERSION="$(python3 - <<'PY'
import pathlib
import re

text = pathlib.Path("Cargo.toml").read_text(encoding="utf-8")
match = re.search(r'(?m)^version = "([^"]+)"', text)
if not match:
    raise SystemExit("workspace version not found in Cargo.toml")
print(match.group(1))
PY
)"
fi

[[ "${HOST}" =~ ^[a-zA-Z0-9_][a-zA-Z0-9_.@:-]*$ ]] || { echo "invalid HOST" >&2; exit 1; }
[[ "${DOMAIN}" =~ ^[a-zA-Z0-9][a-zA-Z0-9.-]*[a-zA-Z0-9]$ ]] || { echo "invalid NEXUSHUB_DOMAIN" >&2; exit 1; }
[[ "${PATH_PREFIX}" =~ ^/[a-zA-Z0-9_/-]+/$ ]] || { echo "invalid path prefix" >&2; exit 1; }
[[ "${EXPECTED_VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "invalid version" >&2; exit 1; }
STAGE="$(mktemp -d "${TMPDIR:-/tmp}/nexushub-deploy.XXXXXXXX")"
REMOTE_STAGE="$(ssh "${HOST}" 'mktemp -d /tmp/nexushub-deploy.XXXXXXXX')"
[[ "${REMOTE_STAGE}" =~ ^/tmp/nexushub-deploy\.[a-zA-Z0-9]+$ ]] || { echo "invalid remote staging path" >&2; exit 1; }
trap 'echo "Deployment staging retained for diagnosis: ${STAGE} and ${REMOTE_STAGE}" >&2' ERR
scp "${ARCHIVE}" "${HOST}:${REMOTE_STAGE}/release.tar.gz"
tar -C deploy -czf "${STAGE}/deploy.tar.gz" nexushub-webd
scp "${STAGE}/deploy.tar.gz" "${HOST}:${REMOTE_STAGE}/deploy.tar.gz"
ssh "${HOST}" "tar -xzf '${REMOTE_STAGE}/deploy.tar.gz' -C '${REMOTE_STAGE}'"
ssh "${HOST}" "sudo -n bash '${REMOTE_STAGE}/nexushub-webd/install.sh' --archive '${REMOTE_STAGE}/release.tar.gz' --domain '${DOMAIN}' --path-prefix '${PATH_PREFIX}'"
ssh "${HOST}" "sudo -n systemctl is-active --quiet nexushub-webd && test \"\$(sudo -n /usr/local/bin/nexushub-webd --version)\" = \"nexushub-webd ${EXPECTED_VERSION}\" && curl -fsS http://127.0.0.1:15742/healthz >/dev/null"

PUBLIC_BASE="https://${DOMAIN%/}${PATH_PREFIX}"
PUBLIC_BASE="${PUBLIC_BASE%/}/"

expect_http_status() {
  local url="$1"
  local expected="$2"
  local status
  status="$(curl -sS -o /dev/null -w "%{http_code}" "${url}")"
  if [[ "${status}" != "${expected}" ]]; then
    echo "unexpected HTTP status for ${url}: got ${status}, expected ${expected}" >&2
    exit 1
  fi
}

expect_404_or_not_nexushub() {
  local url="$1"
  local body
  local status
  local tmp
  tmp="$(mktemp)"
  status="$(curl -sS -o "${tmp}" -w "%{http_code}" "${url}")"
  if [[ "${status}" == "404" ]]; then
    rm -- "${tmp}"
    return 0
  fi
  body="$(tr -d '\000' <"${tmp}")"
  rm -- "${tmp}"
  if grep -Eiq 'nexushub|"label"[[:space:]]*:[[:space:]]*"Probe"|"flavor"[[:space:]]*:[[:space:]]*"builtin"' <<<"${body}"; then
    echo "legacy path appears to be handled by NexusHub: ${url} returned ${status}" >&2
    exit 1
  fi
}

expect_http_status "${PUBLIC_BASE}" "200"
expect_http_status "https://${DOMAIN%/}/codex-cloud-panel/" "404"
expect_404_or_not_nexushub "https://${DOMAIN%/}/api/sentinel/status"
expect_404_or_not_nexushub "https://${DOMAIN%/}/api/probe/status"
expect_404_or_not_nexushub "https://${DOMAIN%/}/api/v1/models"
expect_http_status "${PUBLIC_BASE}api/sentinel/status" "404"
expect_http_status "${PUBLIC_BASE}api/probe/status" "404"

ssh "${HOST}" "rm -r -- '${REMOTE_STAGE}'"
rm -r -- "${STAGE}"
trap - ERR
