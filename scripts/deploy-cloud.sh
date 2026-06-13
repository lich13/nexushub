#!/usr/bin/env bash
set -Eeuo pipefail

HOST="${1:-43.155.235.227}"
ARCHIVE="${2:-dist/nexushub-linux-x86_64.tar.gz}"
DOMAIN="${NEXUSHUB_DOMAIN:-661313.xyz}"
PATH_PREFIX="${NEXUSHUB_PATH_PREFIX:-/nexushub/}"

if [[ ! -f "${ARCHIVE}" ]]; then
  echo "archive not found: ${ARCHIVE}" >&2
  exit 1
fi

REMOTE_ARCHIVE="/tmp/nexushub-linux-x86_64.tar.gz"
scp "${ARCHIVE}" "${HOST}:${REMOTE_ARCHIVE}"
tar -C deploy -czf /tmp/nexushub-deploy.tar.gz nexushub
scp /tmp/nexushub-deploy.tar.gz "${HOST}:/tmp/nexushub-deploy.tar.gz"
ssh "${HOST}" "rm -rf /tmp/nexushub-deploy && mkdir -p /tmp/nexushub-deploy && tar -xzf /tmp/nexushub-deploy.tar.gz -C /tmp/nexushub-deploy"
ssh "${HOST}" "sudo -n bash /tmp/nexushub-deploy/nexushub/install.sh --archive ${REMOTE_ARCHIVE} --domain ${DOMAIN} --path-prefix ${PATH_PREFIX}"
ssh "${HOST}" "sudo -n systemctl is-active nexushub && curl -fsS http://127.0.0.1:15732/healthz"
