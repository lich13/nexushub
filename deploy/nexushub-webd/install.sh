#!/usr/bin/env bash
set -Eeuo pipefail

APP_NAME="nexushub-webd"
BIN_NAME="nexushub-webd"
SERVICE_NAME="nexushub-webd"
INSTALL_BIN="/usr/local/bin/${BIN_NAME}"
SHARE_DIR="/usr/share/${APP_NAME}"
WEBUI_DIR="${SHARE_DIR}/webui"
CONFIG_DIR="/etc/${APP_NAME}"
CONFIG_FILE="${CONFIG_DIR}/config.toml"
ENV_FILE="${CONFIG_DIR}/env"
DATA_DIR="/var/lib/${APP_NAME}"
LOG_DIR="/var/log/${APP_NAME}"
BACKUP_DIR="${DATA_DIR}/backups"
NGINX_BACKUP_DIR="${BACKUP_DIR}/nginx"
SYSTEMD_UNIT="/etc/systemd/system/${SERVICE_NAME}.service"
UPDATE_BIN="/usr/local/bin/${APP_NAME}-update"
CODEX_PRECHECK_WRAPPER_BIN="/usr/local/bin/nexushub-codex-precheck"
CODEX_UPDATE_WRAPPER_BIN="/usr/local/bin/nexushub-codex-update"
CODEX_PRUNE_WRAPPER_BIN="/usr/local/bin/nexushub-codex-prune"
NGINX_SNIPPET="/etc/nginx/snippets/nexushub.conf"
LEGACY_DIR="/opt/nexushub"
LEGACY_SERVICE="nexushub"

ARCHIVE_PATH=""
BINARY_PATH=""
DOMAIN=""
PATH_PREFIX="/nexushub/"
INSTALL_NGINX=0
FORCE_CONFIG=0
ENABLE_SERVICE=1
CHECK_ONLY=0

usage() {
  cat <<'USAGE'
Install NexusHub WebUI daemon.

Usage:
  sudo install.sh --archive ./nexushub-webd-linux-x86_64.tar.gz --domain 661313.xyz --path-prefix /nexushub/
  sudo install.sh --binary ./target/release/nexushub-webd
  install.sh --check

Options:
  --archive PATH       Install release tarball.
  --binary PATH        Install local binary without WebUI.
  --domain DOMAIN      Add nginx snippet include to the matching vhost when possible.
  --path-prefix PATH   Public path prefix. Default: /nexushub/
  --force-config       Replace existing config.toml.
  --no-enable          Do not enable/start service.
  --check              Validate archive layout when run from extracted deploy/.
  -h, --help           Show help.
USAGE
}

log() { printf '[%s] %s\n' "${APP_NAME}" "$*"; }
die() { printf '[%s] ERROR: %s\n' "${APP_NAME}" "$*" >&2; exit 1; }

script_dir() {
  cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd -P
}

archive_root_from_deploy() {
  cd -- "$(script_dir)/.." >/dev/null 2>&1 && pwd -P
}

check_layout() {
  local root
  root="$(archive_root_from_deploy)"
  [[ -x "${root}/bin/${BIN_NAME}" ]] || die "archive missing executable bin/${BIN_NAME}"
  [[ -f "${root}/webui/index.html" ]] || die "archive missing webui/index.html"
  [[ -f "${root}/deploy/install.sh" ]] || die "archive missing deploy/install.sh"
  [[ -f "${root}/deploy/update.sh" ]] || die "archive missing deploy/update.sh"
  [[ -f "${root}/deploy/systemd.service" ]] || die "archive missing deploy/systemd.service"
  "${root}/bin/${BIN_NAME}" --version | grep -Eq '^nexushub-webd [0-9]+\.[0-9]+\.[0-9]+'
  echo "nexushub-webd archive layout ok"
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --archive) ARCHIVE_PATH="${2:-}"; shift 2 ;;
      --binary) BINARY_PATH="${2:-}"; shift 2 ;;
      --domain) DOMAIN="${2:-}"; INSTALL_NGINX=1; shift 2 ;;
      --path-prefix) PATH_PREFIX="${2:-}"; shift 2 ;;
      --force-config) FORCE_CONFIG=1; shift ;;
      --no-enable) ENABLE_SERVICE=0; shift ;;
      --check) CHECK_ONLY=1; shift ;;
      -h|--help) usage; exit 0 ;;
      *) die "unknown argument: $1" ;;
    esac
  done
  if [[ "${CHECK_ONLY}" -eq 1 ]]; then
    return
  fi
  [[ -n "${ARCHIVE_PATH}" || -n "${BINARY_PATH}" ]] || die "pass --archive or --binary"
  [[ "${PATH_PREFIX}" == /*/ ]] || die "--path-prefix must look like /name/"
}

require_root() {
  [[ "${EUID}" -eq 0 ]] || die "run as root"
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

install_dirs() {
  install -d -m 0755 -o root -g root /usr/local/bin "${SHARE_DIR}" "${WEBUI_DIR}" "${CONFIG_DIR}"
  install -d -m 0750 -o root -g root "${DATA_DIR}" "${LOG_DIR}" "${BACKUP_DIR}" "${NGINX_BACKUP_DIR}"
}

copy_legacy_runtime_once() {
  if [[ -f "${LEGACY_DIR}/config.toml" && ! -f "${CONFIG_FILE}" ]]; then
    install -m 0640 -o root -g root "${LEGACY_DIR}/config.toml" "${CONFIG_FILE}"
  fi
  if [[ -f "${LEGACY_DIR}/env" && ! -f "${ENV_FILE}" ]]; then
    install -m 0640 -o root -g root "${LEGACY_DIR}/env" "${ENV_FILE}"
  fi
  if [[ -f "${LEGACY_DIR}/nexushub.sqlite" && ! -f "${DATA_DIR}/nexushub.sqlite" ]]; then
    install -m 0640 -o root -g root "${LEGACY_DIR}/nexushub.sqlite" "${DATA_DIR}/nexushub.sqlite"
  fi
  for suffix in -wal -shm; do
    if [[ -f "${LEGACY_DIR}/nexushub.sqlite${suffix}" && ! -f "${DATA_DIR}/nexushub.sqlite${suffix}" ]]; then
      install -m 0640 -o root -g root "${LEGACY_DIR}/nexushub.sqlite${suffix}" "${DATA_DIR}/nexushub.sqlite${suffix}"
    fi
  done
}

install_payload() {
  if [[ -n "${BINARY_PATH}" ]]; then
    [[ -f "${BINARY_PATH}" ]] || die "binary not found: ${BINARY_PATH}"
    install -m 0755 -o root -g root "${BINARY_PATH}" "${INSTALL_BIN}"
    return
  fi
  [[ -f "${ARCHIVE_PATH}" ]] || die "archive not found: ${ARCHIVE_PATH}"
  local tmp root
  tmp="$(mktemp -d)"
  trap 'rm -rf "${tmp}"' RETURN
  tar -xzf "${ARCHIVE_PATH}" -C "${tmp}"
  root="$(find "${tmp}" -maxdepth 1 -type d -name 'nexushub-webd-linux-*' | head -n 1)"
  [[ -n "${root}" ]] || die "archive root nexushub-webd-linux-* not found"
  [[ -x "${root}/bin/${BIN_NAME}" ]] || die "archive missing bin/${BIN_NAME}"
  [[ -f "${root}/webui/index.html" ]] || die "archive missing webui/index.html"

  install -m 0755 -o root -g root "${root}/bin/${BIN_NAME}" "${INSTALL_BIN}"
  find "${WEBUI_DIR}" -mindepth 1 -maxdepth 1 -exec rm -rf {} +
  cp -a "${root}/webui/." "${WEBUI_DIR}/"
  chown -R root:root "${WEBUI_DIR}"

  if [[ -d "${root}/deploy" ]]; then
    [[ -f "${root}/deploy/update.sh" ]] && install -m 0755 -o root -g root "${root}/deploy/update.sh" "${UPDATE_BIN}"
    [[ -f "${root}/deploy/nexushub-codex-precheck" ]] && install -m 0755 -o root -g root "${root}/deploy/nexushub-codex-precheck" "${CODEX_PRECHECK_WRAPPER_BIN}"
    [[ -f "${root}/deploy/nexushub-codex-update" ]] && install -m 0755 -o root -g root "${root}/deploy/nexushub-codex-update" "${CODEX_UPDATE_WRAPPER_BIN}"
    [[ -f "${root}/deploy/nexushub-codex-prune" ]] && install -m 0755 -o root -g root "${root}/deploy/nexushub-codex-prune" "${CODEX_PRUNE_WRAPPER_BIN}"
  fi
  trap - RETURN
}

install_codex_wrappers() {
  local source_dir
  source_dir="$(script_dir)"
  install -m 0755 -o root -g root "${source_dir}/nexushub-codex-precheck" "${CODEX_PRECHECK_WRAPPER_BIN}"
  install -m 0755 -o root -g root "${source_dir}/nexushub-codex-update" "${CODEX_UPDATE_WRAPPER_BIN}"
  install -m 0755 -o root -g root "${source_dir}/nexushub-codex-prune" "${CODEX_PRUNE_WRAPPER_BIN}"
}

install_codex_home_write_paths() {
  install -d -m 0700 -o root -g root /root/.codex

  local ubuntu_owner="root"
  local ubuntu_group="root"
  if getent passwd ubuntu >/dev/null 2>&1; then
    ubuntu_owner="ubuntu"
    ubuntu_group="$(id -gn ubuntu 2>/dev/null || printf 'ubuntu')"
  fi

  [[ -d /home ]] || install -d -m 0755 -o root -g root /home
  [[ -d /home/ubuntu ]] || install -d -m 0755 -o "${ubuntu_owner}" -g "${ubuntu_group}" /home/ubuntu
  install -d -m 0700 -o "${ubuntu_owner}" -g "${ubuntu_group}" /home/ubuntu/.codex
}

install_config() {
  local source_dir
  source_dir="$(script_dir)"
  copy_legacy_runtime_once
  if [[ ! -f "${CONFIG_FILE}" || "${FORCE_CONFIG}" -eq 1 ]]; then
    install -m 0640 -o root -g root "${source_dir}/config.example.toml" "${CONFIG_FILE}"
    if [[ -n "${DOMAIN}" ]]; then
      sed -i "s#https://661313.xyz/nexushub/#https://${DOMAIN}${PATH_PREFIX}#g" "${CONFIG_FILE}"
    fi
  fi
  if [[ ! -f "${ENV_FILE}" ]]; then
    install -m 0640 -o root -g root "${source_dir}/env.example" "${ENV_FILE}"
  fi
  ensure_secret_key
  ensure_config_defaults
}

ensure_secret_key() {
  if grep -q '^NEXUSHUB_SECRET_KEY=' "${ENV_FILE}" 2>/dev/null; then
    return
  fi

  read_legacy_secret() {
    local key="$1"
    local path="$2"
    [[ -f "${path}" ]] || return 0
    awk -v want="${key}" '
      BEGIN { FS="=" }
      $1 == want {
        value=$0
        sub(/^[^=]*=/, "", value)
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
        if (value ~ /^".*"$/ || value ~ /^\047.*\047$/) {
          value=substr(value, 2, length(value)-2)
        }
        print value
        exit
      }
    ' "${path}"
  }

  local secret=""
  if [[ -f "${LEGACY_DIR}/env" ]]; then
    secret="$(read_legacy_secret NEXUSHUB_SECRET_KEY "${LEGACY_DIR}/env")"
  fi
  if [[ -z "${secret}" && -f /etc/codex-cloud-panel/env ]]; then
    secret="$(read_legacy_secret CODEX_CLOUD_PANEL_SECRET_KEY /etc/codex-cloud-panel/env)"
  fi
  if [[ -z "${secret}" && -f /etc/cc-switch-lite/env ]]; then
    secret="$(read_legacy_secret CC_SWITCH_LITE_SECRET_KEY /etc/cc-switch-lite/env)"
  fi
  if [[ -z "${secret}" ]]; then
    secret="$(python3 - <<'PY'
import secrets
print(secrets.token_urlsafe(32))
PY
)"
  fi
  {
    printf '\n'
    printf 'NEXUSHUB_SECRET_KEY=%s\n' "${secret}"
  } >> "${ENV_FILE}"
  chmod 0640 "${ENV_FILE}"
  chown root:root "${ENV_FILE}"
}

ensure_config_defaults() {
  python3 - "${CONFIG_FILE}" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text()
lines = text.splitlines()

def is_legacy_codex_precheck(key, value):
    return key == "precheck_command" and all(
        needle in value
        for needle in (
            "codex --version",
            "sudo -n codex --version",
            "/usr/local/bin/codex-raw --version",
            "/root/.codex/state_5.sqlite",
            "/home/ubuntu/codex-admin/bin/codex-cloud-doctor",
        )
    )

def ensure_section(section, required_values, replace_values=None):
    global lines
    replace_values = replace_values or {}
    start = None
    end = len(lines)
    for index, line in enumerate(lines):
        if line.strip() == f"[{section}]":
            start = index
            continue
        if start is not None and index > start and line.strip().startswith("[") and line.strip().endswith("]"):
            end = index
            break
    if start is None:
        if lines and lines[-1].strip():
            lines.append("")
        lines.append(f"[{section}]")
        start = len(lines) - 1
        end = len(lines)
    seen = set()
    for index in range(start + 1, end):
        stripped = lines[index].strip()
        if not stripped or stripped.startswith("#") or "=" not in stripped:
            continue
        key, value = [part.strip() for part in stripped.split("=", 1)]
        if (key in replace_values and value in replace_values[key]) or is_legacy_codex_precheck(key, value):
            lines[index] = f"{key} = {required_values[key]}"
        if key in required_values:
            seen.add(key)
    insert_at = end
    for key, value in required_values.items():
        if key not in seen:
            lines.insert(insert_at, f"{key} = {value}")
            insert_at += 1

def remove_section_keys(section, keys):
    global lines
    start = None
    end = len(lines)
    for index, line in enumerate(lines):
        if line.strip() == f"[{section}]":
            start = index
            continue
        if start is not None and index > start and line.strip().startswith("[") and line.strip().endswith("]"):
            end = index
            break
    if start is None:
        return
    filtered = []
    for index, line in enumerate(lines):
        if start < index < end:
            stripped = line.strip()
            if stripped and not stripped.startswith("#") and "=" in stripped:
                key = stripped.split("=", 1)[0].strip()
                if key in keys:
                    continue
        filtered.append(line)
    lines = filtered

ensure_section("codex", {"host_label": '"43.155.235.227"'})
remove_section_keys(
    "codex",
    {
        "app_server_service",
        "app_server_socket",
        "bridge_enabled",
        "bridge_transport",
        "bridge_timeout_seconds",
    },
)
ensure_section("server", {"listen": '"127.0.0.1:15742"'}, {"listen": {'"127.0.0.1:15732"'}})
ensure_section(
    "security",
    {
        "session_ttl_seconds": "31536000",
        "turnstile_expected_hostname": '"661313.xyz"',
        "turnstile_expected_action": '"login"',
    },
    {"session_ttl_seconds": {"604800"}},
)
ensure_section(
    "paths",
    {
        "data_dir": '"/var/lib/nexushub-webd"',
        "db_path": '"/var/lib/nexushub-webd/nexushub.sqlite"',
        "webui_dir": '"/usr/share/nexushub-webd/webui"',
        "log_dir": '"/var/log/nexushub-webd"',
    },
    {
        "data_dir": {'"/var/lib/codex-cloud-panel"', '"/opt/codex-cloud-panel"', '"/opt/nexushub"'},
        "db_path": {
            '"/var/lib/codex-cloud-panel/panel.sqlite"',
            '"/var/lib/codex-cloud-panel/codex-cloud-panel.sqlite"',
            '"/opt/codex-cloud-panel/codex-cloud-panel.sqlite"',
            '"/opt/nexushub/nexushub.sqlite"',
        },
        "webui_dir": {'"/usr/share/codex-cloud-panel/webui"', '"/opt/codex-cloud-panel/webui"', '"/opt/nexushub/webui"'},
        "log_dir": {'"/var/log/codex-cloud-panel"', '"/opt/codex-cloud-panel/logs"', '"/opt/nexushub/logs"'},
    },
)
ensure_section(
    "update",
    {
        "precheck_command": '"/usr/local/bin/nexushub-codex-precheck"',
        "update_command": '"/usr/local/bin/nexushub-codex-update"',
        "prune_command": '"/usr/local/bin/nexushub-codex-prune"',
        "panel_update_command": '"/usr/local/bin/nexushub-webd-update --repo lich13/nexushub --version latest"',
        "panel_precheck_command": '"test -x /usr/local/bin/nexushub-webd-update && systemctl is-active nexushub-webd && curl -fsS http://127.0.0.1:15742/healthz"',
    },
    {
        "update_command": {
            '"sudo -n /home/ubuntu/codex-admin/bin/codex-cloud-update --no-prune"',
            '"/home/ubuntu/codex-admin/bin/codex-cloud-update --no-prune"',
        },
        "prune_command": {
            '"sudo -n /home/ubuntu/codex-admin/bin/codex-cloud-prune"',
            '"/home/ubuntu/codex-admin/bin/codex-cloud-prune"',
        },
        "panel_update_command": {
            '"/usr/local/bin/nexushub-update --repo lich13/nexushub --version latest"',
        },
        "panel_precheck_command": {
            '"/usr/local/bin/nexushub-update --precheck"',
            '"test -x /usr/local/bin/nexushub-update && systemctl is-active nexushub && curl -fsS http://127.0.0.1:15742/healthz"',
            '"test -x /usr/local/bin/nexushub-update && systemctl is-active nexushub && curl -fsS http://127.0.0.1:15732/healthz"',
        },
    },
)
ensure_section("probe", {"enabled": "true", "poll_seconds": "15", "recent_limit": "50"})
ensure_section("probe.hooks", {"manage_stop_hook": "true"})
remove_section_keys("probe.hooks", {"reload_app_server_after_install"})
ensure_section(
    "probe.notifications",
    {
        "enabled": "false",
        "server_url": '"https://api.day.app"',
        "group": '"NexusHub"',
        "notify_completion": "true",
        "notify_reply_needed": "true",
        "notify_recoverable": "true",
    },
)
ensure_section(
    "probe.observability",
    {"hook_event_max_lines": "500", "hook_cooldown_max_lines": "1000", "log_max_bytes": "5242880"},
    {"hook_event_max_lines": {"120"}, "hook_cooldown_max_lines": {"80"}, "log_max_bytes": {"262144"}},
)
ensure_section(
    "probe.logs_db",
    {
        "enabled": "true",
        "retention_days": "2",
        "maintenance_interval_hours": "6",
        "maintain_on_codex_exit": "true",
        "codex_exit_grace_seconds": "5",
        "codex_exit_max_wait_seconds": "1800",
        "delete_chunk_rows": "5000",
        "max_delete_rows_per_run": "100000",
        "busy_timeout_ms": "500",
        "auto_compact_when_codex_closed": "true",
        "compact_interval_hours": "24",
        "compact_min_freelist_mb": "256",
        "compact_min_freelist_ratio_percent": "20",
        "minimum_free_space_mb": "1024",
    },
    {
        "retention_days": {"14"},
        "maintenance_interval_hours": {"24"},
        "codex_exit_grace_seconds": {"10"},
        "codex_exit_max_wait_seconds": {"120"},
        "delete_chunk_rows": {"2000"},
        "max_delete_rows_per_run": {"50000"},
        "busy_timeout_ms": {"5000"},
        "compact_interval_hours": {"168"},
        "compact_min_freelist_mb": {"64"},
        "minimum_free_space_mb": {"256"},
    },
)

legacy_host = '"' + "-".join(["tencent", "wanka"]) + '"'
lines = [line.replace(legacy_host, '"43.155.235.227"') for line in lines]
path.write_text("\n".join(lines) + "\n")
PY
}

install_systemd() {
  local source_dir
  source_dir="$(script_dir)"
  if systemctl list-unit-files "${LEGACY_SERVICE}.service" >/dev/null 2>&1; then
    systemctl stop "${LEGACY_SERVICE}.service" >/dev/null 2>&1 || true
    systemctl disable "${LEGACY_SERVICE}.service" >/dev/null 2>&1 || true
  fi
  install -m 0644 -o root -g root "${source_dir}/systemd.service" "${SYSTEMD_UNIT}"
  systemctl daemon-reload
  if [[ "${ENABLE_SERVICE}" -eq 1 ]]; then
    systemctl enable "${SERVICE_NAME}.service"
    systemctl restart "${SERVICE_NAME}.service"
  fi
}

install_nginx() {
  [[ "${INSTALL_NGINX}" -eq 1 ]] || return 0
  require_command nginx
  local source_dir snippet target
  source_dir="$(script_dir)"
  snippet="$(mktemp)"
  sed "s#/nexushub/#${PATH_PREFIX}#g" "${source_dir}/nginx.conf" > "${snippet}"
  install -m 0644 -o root -g root "${snippet}" "${NGINX_SNIPPET}"
  target="$(grep -Rsl "server_name .*${DOMAIN}" /etc/nginx/sites-enabled /etc/nginx/sites-available 2>/dev/null | head -n 1 || true)"
  if [[ -n "${target}" ]] && ! grep -Fq "${NGINX_SNIPPET}" "${target}"; then
    cp "${target}" "${NGINX_BACKUP_DIR}/$(basename "${target}").bak-$(date +%Y%m%d%H%M%S)"
    python3 - "$target" "$NGINX_SNIPPET" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
snippet = sys.argv[2]
text = path.read_text()
marker = f"    include {snippet};\n"
idx = text.rfind("\n}")
if idx == -1:
    raise SystemExit("could not find server block close")
path.write_text(text[:idx] + "\n" + marker + text[idx:])
PY
  fi
  nginx -t
  systemctl reload nginx
}

main() {
  parse_args "$@"
  if [[ "${CHECK_ONLY}" -eq 1 ]]; then
    check_layout
    return
  fi
  require_root
  require_command tar
  install_dirs
  install_payload
  install_codex_wrappers
  install_config
  install_codex_home_write_paths
  install_systemd
  install_nginx
  log "installed ${INSTALL_BIN}"
}

main "$@"
