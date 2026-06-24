#!/usr/bin/env bash
set -Eeuo pipefail

HOST="${1:-43.155.235.227}"
ARCHIVE="${2:-dist/nexushub-linux-x86_64.tar.gz}"
DOMAIN="${NEXUSHUB_DOMAIN:-661313.xyz}"
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

REMOTE_ARCHIVE="/tmp/nexushub-linux-x86_64.tar.gz"
scp "${ARCHIVE}" "${HOST}:${REMOTE_ARCHIVE}"
tar -C deploy -czf /tmp/nexushub-deploy.tar.gz nexushub
scp /tmp/nexushub-deploy.tar.gz "${HOST}:/tmp/nexushub-deploy.tar.gz"
ssh "${HOST}" "rm -rf /tmp/nexushub-deploy && mkdir -p /tmp/nexushub-deploy && tar -xzf /tmp/nexushub-deploy.tar.gz -C /tmp/nexushub-deploy"
ssh "${HOST}" "sudo -n bash /tmp/nexushub-deploy/nexushub/install.sh --archive ${REMOTE_ARCHIVE} --domain ${DOMAIN} --path-prefix ${PATH_PREFIX}"
ssh "${HOST}" "sudo -n systemctl is-active --quiet nexushub && test \"\$(sudo -n /opt/nexushub/bin/nexushubd --version)\" = \"nexushubd ${EXPECTED_VERSION}\" && curl -fsS http://127.0.0.1:15742/healthz >/dev/null"

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
    rm -f "${tmp}"
    return 0
  fi
  body="$(tr -d '\000' <"${tmp}")"
  rm -f "${tmp}"
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
