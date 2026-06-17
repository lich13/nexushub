#!/usr/bin/env bash
set -Eeuo pipefail

APP_NAME="NexusHub"
SERVICE_LABEL="com.nexushub.nexushub"
APP_DIR="${HOME}/Library/Application Support/NexusHub"
BIN_DIR="${APP_DIR}/bin"
WEBUI_DIR="${APP_DIR}/webui"
LOG_DIR="${HOME}/Library/Logs/NexusHub"
LAUNCH_AGENTS_DIR="${HOME}/Library/LaunchAgents"
PLIST_PATH="${LAUNCH_AGENTS_DIR}/${SERVICE_LABEL}.plist"
CONFIG_FILE="${APP_DIR}/config.toml"
ENV_FILE="${APP_DIR}/env"

SOURCE_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd -P)"
PAYLOAD_DIR="${SOURCE_DIR}"
if [[ -d "${SOURCE_DIR}/NexusHub" ]]; then
  PAYLOAD_DIR="${SOURCE_DIR}/NexusHub"
elif [[ "$(basename "${SOURCE_DIR}")" == "macos" && -d "${SOURCE_DIR}/../.." ]]; then
  PAYLOAD_DIR="$(cd -- "${SOURCE_DIR}/../.." >/dev/null 2>&1 && pwd -P)"
fi

log() { printf '[%s] %s\n' "${APP_NAME}" "$*"; }
die() { printf '[%s] ERROR: %s\n' "${APP_NAME}" "$*" >&2; exit 1; }

secret_key() {
  python3 - <<'PY'
import secrets
print(secrets.token_urlsafe(32))
PY
}

toml_escape() {
  python3 - "$1" <<'PY'
import sys
value = sys.argv[1]
print(value.replace("\\", "\\\\").replace('"', '\\"'))
PY
}

write_default_config() {
  local app_dir db_path webui_dir log_dir
  app_dir="$(toml_escape "${APP_DIR}")"
  db_path="$(toml_escape "${APP_DIR}/nexushub.sqlite")"
  webui_dir="$(toml_escape "${WEBUI_DIR}")"
  log_dir="$(toml_escape "${LOG_DIR}")"

  cat > "${CONFIG_FILE}" <<EOF_CONFIG
[server]
listen = "127.0.0.1:15742"
public_base_url = "http://127.0.0.1:15742/nexushub/"
trust_forwarded_headers = true

[codex]
workspace = "${app_dir}/workspace"
host_label = "macOS"

[probe]
enabled = true
poll_seconds = 15
recent_limit = 50

[probe.hooks]
manage_stop_hook = false
reload_app_server_after_install = false

[probe.notifications]
enabled = false
server_url = "https://api.day.app"
group = "NexusHub"
notify_completion = true
notify_reply_needed = true
notify_recoverable = true

[probe.observability]
hook_event_max_lines = 500
hook_cooldown_max_lines = 1000
log_max_bytes = 5242880

[probe.logs_db]
enabled = true
retention_days = 2
maintenance_interval_hours = 6
maintain_on_codex_exit = true
codex_exit_grace_seconds = 5
codex_exit_max_wait_seconds = 1800
delete_chunk_rows = 5000
max_delete_rows_per_run = 100000
busy_timeout_ms = 500
auto_compact_when_codex_closed = true
compact_interval_hours = 24
compact_min_freelist_mb = 256
compact_min_freelist_ratio_percent = 20
minimum_free_space_mb = 1024

[security]
cookie_secure = false
session_ttl_seconds = 31536000
login_rate_limit_per_minute = 8

[paths]
data_dir = "${app_dir}"
db_path = "${db_path}"
webui_dir = "${webui_dir}"
log_dir = "${log_dir}"

[update]
precheck_command = ""
update_command = ""
prune_command = ""
doctor_command = ""
panel_update_command = ""
panel_precheck_command = ""
EOF_CONFIG
}

ensure_env() {
  if [[ -f "${ENV_FILE}" ]] && grep -q '^NEXUSHUB_SECRET_KEY=' "${ENV_FILE}"; then
    return
  fi
  {
    printf 'RUST_LOG=nexushubd=info,tower_http=info\n'
    printf 'NEXUSHUB_SECRET_KEY=%s\n' "$(secret_key)"
  } > "${ENV_FILE}"
  chmod 0600 "${ENV_FILE}"
}

install_payload() {
  [[ -x "${PAYLOAD_DIR}/bin/nexushubd" ]] || die "missing payload binary: ${PAYLOAD_DIR}/bin/nexushubd"
  if [[ -d "${PAYLOAD_DIR}/webui/dist" ]]; then
    WEBUI_SOURCE="${PAYLOAD_DIR}/webui/dist"
  elif [[ -d "${PAYLOAD_DIR}/webui" ]]; then
    WEBUI_SOURCE="${PAYLOAD_DIR}/webui"
  else
    die "missing payload webui"
  fi

  mkdir -p "${BIN_DIR}" "${WEBUI_DIR}" "${LOG_DIR}" "${LAUNCH_AGENTS_DIR}" "${APP_DIR}/workspace"
  install -m 0755 "${PAYLOAD_DIR}/bin/nexushubd" "${BIN_DIR}/nexushubd"
  rm -rf "${WEBUI_DIR}"
  mkdir -p "${WEBUI_DIR}"
  cp -a "${WEBUI_SOURCE}/." "${WEBUI_DIR}/"
}

install_config() {
  if [[ ! -f "${CONFIG_FILE}" ]]; then
    write_default_config
    chmod 0600 "${CONFIG_FILE}"
  fi
  ensure_env
}

install_launch_agent() {
  local template="${PAYLOAD_DIR}/com.nexushub.nexushub.plist"
  [[ -f "${template}" ]] || die "missing LaunchAgent template: ${template}"
  sed \
    -e "s#__APP_DIR__#${APP_DIR//\\/\\\\}#g" \
    -e "s#__LOG_DIR__#${LOG_DIR//\\/\\\\}#g" \
    "${template}" > "${PLIST_PATH}"
  chmod 0644 "${PLIST_PATH}"

  if [[ "${NEXUSHUB_SKIP_LAUNCH:-0}" == "1" ]]; then
    return
  fi
  launchctl bootout gui/$(id -u) "${PLIST_PATH}" >/dev/null 2>&1 || true
  launchctl bootstrap gui/$(id -u) "${PLIST_PATH}"
  launchctl enable gui/$(id -u)/com.nexushub.nexushub
  launchctl kickstart -k gui/$(id -u)/com.nexushub.nexushub
}

main() {
  install_payload
  install_config
  install_launch_agent
  log "installed user LaunchAgent at ${PLIST_PATH}"
  log "service listens on http://127.0.0.1:15742/nexushub/"
}

main "$@"
