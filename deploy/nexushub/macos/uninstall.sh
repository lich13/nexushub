#!/usr/bin/env bash
set -Eeuo pipefail

APP_NAME="NexusHub"
SERVICE_LABEL="com.nexushub.nexushub"
APP_DIR="${HOME}/Library/Application Support/NexusHub"
LOG_DIR="${HOME}/Library/Logs/NexusHub"
LAUNCH_AGENTS_DIR="${HOME}/Library/LaunchAgents"
PLIST_PATH="${LAUNCH_AGENTS_DIR}/com.nexushub.nexushub.plist"
REMOVE_DATA="${REMOVE_DATA:-0}"

log() { printf '[%s] %s\n' "${APP_NAME}" "$*"; }

if [[ "${NEXUSHUB_SKIP_LAUNCH:-0}" != "1" ]]; then
  launchctl bootout gui/$(id -u) "${PLIST_PATH}" >/dev/null 2>&1 || true
  launchctl disable gui/$(id -u)/com.nexushub.nexushub >/dev/null 2>&1 || true
fi

rm -f "${PLIST_PATH}"

if [[ "${REMOVE_DATA}" == "1" ]]; then
  rm -rf "${APP_DIR}" "${LOG_DIR}"
  log "removed LaunchAgent, application data, and logs"
else
  rm -rf "${APP_DIR}/bin" "${APP_DIR}/webui"
  log "removed LaunchAgent and installed binaries; kept ${APP_DIR} data and ${LOG_DIR} logs"
fi
