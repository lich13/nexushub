#!/usr/bin/env bash
set -Eeuo pipefail

APP_NAME="nexushub"
SERVICE_NAME="nexushub"
BIN_NAME="nexushubd"
INSTALL_DIR="/opt/${APP_NAME}"
INSTALL_BIN="${INSTALL_DIR}/bin/${BIN_NAME}"
CONFIG_DIR="${INSTALL_DIR}"
CONFIG_FILE="${CONFIG_DIR}/config.toml"
ENV_FILE="${CONFIG_DIR}/env"
DATA_DIR="${INSTALL_DIR}"
BACKUP_DIR="${DATA_DIR}/backups"
NGINX_BACKUP_DIR="${BACKUP_DIR}/nginx"
LOG_DIR="${INSTALL_DIR}/logs"
WEBUI_DIR="${INSTALL_DIR}/webui"
SYSTEMD_UNIT="/etc/systemd/system/${SERVICE_NAME}.service"
UPDATE_BIN="/usr/local/bin/${APP_NAME}-update"
CODEX_PRECHECK_WRAPPER_BIN="/usr/local/bin/${APP_NAME}-codex-precheck"
CODEX_UPDATE_WRAPPER_BIN="/usr/local/bin/${APP_NAME}-codex-update"
CODEX_PRUNE_WRAPPER_BIN="/usr/local/bin/${APP_NAME}-codex-prune"
NGINX_SNIPPET="/etc/nginx/snippets/${APP_NAME}.conf"

ARCHIVE_PATH=""
BINARY_PATH=""
DOMAIN=""
PATH_PREFIX="/nexushub/"
INSTALL_NGINX=0
FORCE_CONFIG=0
ENABLE_SERVICE=1

usage() {
  cat <<'USAGE'
Install NexusHub.

Usage:
  sudo install.sh --archive ./nexushub-linux-x86_64.tar.gz --domain 661313.xyz --path-prefix /nexushub/

Options:
  --archive PATH       Install release tarball.
  --binary PATH        Install local binary without WebUI.
  --domain DOMAIN      Add nginx snippet include to the matching vhost when possible.
  --path-prefix PATH   Public path prefix. Default: /nexushub/
  --force-config       Replace existing config.toml.
  --no-enable          Do not enable/start service.
  -h, --help           Show help.
USAGE
}

log() { printf '[%s] %s\n' "${APP_NAME}" "$*"; }
die() { printf '[%s] ERROR: %s\n' "${APP_NAME}" "$*" >&2; exit 1; }

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --archive) ARCHIVE_PATH="${2:-}"; shift 2 ;;
      --binary) BINARY_PATH="${2:-}"; shift 2 ;;
      --domain) DOMAIN="${2:-}"; INSTALL_NGINX=1; shift 2 ;;
      --path-prefix) PATH_PREFIX="${2:-}"; shift 2 ;;
      --force-config) FORCE_CONFIG=1; shift ;;
      --no-enable) ENABLE_SERVICE=0; shift ;;
      -h|--help) usage; exit 0 ;;
      *) die "unknown argument: $1" ;;
    esac
  done
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
  install -d -m 0755 -o root -g root "${INSTALL_DIR}/bin" "$(dirname "${WEBUI_DIR}")" "${WEBUI_DIR}"
  install -d -m 0750 -o root -g root "${CONFIG_DIR}" "${DATA_DIR}" "${BACKUP_DIR}" "${NGINX_BACKUP_DIR}" "${LOG_DIR}"
}

install_payload() {
  if [[ -n "${BINARY_PATH}" ]]; then
    [[ -f "${BINARY_PATH}" ]] || die "binary not found: ${BINARY_PATH}"
    install -m 0755 -o root -g root "${BINARY_PATH}" "${INSTALL_BIN}"
    return
  fi
  [[ -f "${ARCHIVE_PATH}" ]] || die "archive not found: ${ARCHIVE_PATH}"
  local tmp
  tmp="$(mktemp -d)"
  tar -xzf "${ARCHIVE_PATH}" -C "${tmp}"
  local root="${tmp}/${APP_NAME}"
  [[ -x "${root}/bin/${BIN_NAME}" ]] || die "archive missing bin/${BIN_NAME}"
  install -m 0755 -o root -g root "${root}/bin/${BIN_NAME}" "${INSTALL_BIN}"
  if [[ -d "${root}/webui" ]]; then
    rm -rf "${WEBUI_DIR}"
    install -d -m 0755 -o root -g root "${WEBUI_DIR}"
    cp -a "${root}/webui/." "${WEBUI_DIR}/"
    chown -R root:root "${WEBUI_DIR}"
  fi
  if [[ -d "${root}/deploy" ]]; then
    if [[ -f "${root}/deploy/update.sh" ]]; then
      install -m 0755 -o root -g root "${root}/deploy/update.sh" "${UPDATE_BIN}"
    fi
    if [[ -f "${root}/deploy/${APP_NAME}-codex-precheck" ]]; then
      install -m 0755 -o root -g root "${root}/deploy/${APP_NAME}-codex-precheck" "${CODEX_PRECHECK_WRAPPER_BIN}"
    fi
    if [[ -f "${root}/deploy/${APP_NAME}-codex-update" ]]; then
      install -m 0755 -o root -g root "${root}/deploy/${APP_NAME}-codex-update" "${CODEX_UPDATE_WRAPPER_BIN}"
    fi
    if [[ -f "${root}/deploy/${APP_NAME}-codex-prune" ]]; then
      install -m 0755 -o root -g root "${root}/deploy/${APP_NAME}-codex-prune" "${CODEX_PRUNE_WRAPPER_BIN}"
    fi
  fi
}

install_codex_wrappers() {
  local source_dir
  source_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd -P)"
  install -m 0755 -o root -g root "${source_dir}/${APP_NAME}-codex-precheck" "${CODEX_PRECHECK_WRAPPER_BIN}"
  install -m 0755 -o root -g root "${source_dir}/${APP_NAME}-codex-update" "${CODEX_UPDATE_WRAPPER_BIN}"
  install -m 0755 -o root -g root "${source_dir}/${APP_NAME}-codex-prune" "${CODEX_PRUNE_WRAPPER_BIN}"
}

install_config() {
  local source_dir
  source_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd -P)"
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
  ensure_bridge_config
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
  if [[ -f /etc/codex-cloud-panel/env ]]; then
    secret="$(read_legacy_secret CODEX_CLOUD_PANEL_SECRET_KEY /etc/codex-cloud-panel/env)"
  fi
  if [[ -f /etc/cc-switch-lite/env ]]; then
    if [[ -z "${secret}" ]]; then
      secret="$(read_legacy_secret CC_SWITCH_LITE_SECRET_KEY /etc/cc-switch-lite/env)"
    fi
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

ensure_bridge_config() {
  python3 - "${CONFIG_FILE}" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text()
lines = text.splitlines()
section_start = None
section_end = len(lines)

for index, line in enumerate(lines):
    if line.strip() == "[codex]":
        section_start = index
        continue
    if section_start is not None and index > section_start and line.strip().startswith("[") and line.strip().endswith("]"):
        section_end = index
        break

if section_start is None:
    if lines and lines[-1].strip():
        lines.append("")
    lines.append("[codex]")
    section_start = len(lines) - 1
    section_end = len(lines)

required = {
    "host_label": '"43.155.235.227"',
    "app_server_socket": '"\\/root\\/.codex\\/app-server-control\\/app-server-control.sock"'.replace("\\/", "/"),
    "bridge_enabled": "true",
    "bridge_transport": '"websocket"',
    "bridge_timeout_seconds": "20",
}
seen = set()
for index in range(section_start + 1, section_end):
    stripped = lines[index].strip()
    if not stripped or stripped.startswith("#") or "=" not in stripped:
        continue
    key = stripped.split("=", 1)[0].strip()
    if key in required:
        lines[index] = f"{key} = {required[key]}"
        seen.add(key)

insert_at = section_end
for key, value in required.items():
    if key not in seen:
        lines.insert(insert_at, f"{key} = {value}")
        insert_at += 1

def is_legacy_codex_precheck(key, value):
    if key != "precheck_command":
        return False
    return all(
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

ensure_section(
    "server",
    {
        "listen": '"127.0.0.1:15742"',
    },
    {
        "listen": {
            '"127.0.0.1:15732"',
        },
    },
)
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
        "data_dir": '"/opt/nexushub"',
        "db_path": '"/opt/nexushub/nexushub.sqlite"',
        "webui_dir": '"/opt/nexushub/webui"',
        "log_dir": '"/opt/nexushub/logs"',
    },
    {
        "data_dir": {
            '"/var/lib/codex-cloud-panel"',
            '"/opt/codex-cloud-panel"',
        },
        "db_path": {
            '"/var/lib/codex-cloud-panel/panel.sqlite"',
            '"/var/lib/codex-cloud-panel/codex-cloud-panel.sqlite"',
            '"/opt/codex-cloud-panel/codex-cloud-panel.sqlite"',
        },
        "webui_dir": {
            '"/usr/share/codex-cloud-panel/webui"',
            '"/opt/codex-cloud-panel/webui"',
        },
        "log_dir": {
            '"/var/log/codex-cloud-panel"',
            '"/opt/codex-cloud-panel/logs"',
        },
    },
)
ensure_section(
    "update",
    {
        "precheck_command": '"/usr/local/bin/nexushub-codex-precheck"',
        "update_command": '"/usr/local/bin/nexushub-codex-update"',
        "prune_command": '"/usr/local/bin/nexushub-codex-prune"',
        "panel_update_command": '"/usr/local/bin/nexushub-update --repo lich13/nexushub --version latest"',
        "panel_precheck_command": '"test -x /usr/local/bin/nexushub-update && systemctl is-active nexushub && curl -fsS http://127.0.0.1:15742/healthz"',
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
        "panel_precheck_command": {
            '"test -x /usr/local/bin/nexushub-update && systemctl is-active nexushub && curl -fsS http://127.0.0.1:15732/healthz"',
        },
    },
)

legacy_host = '"' + "-".join(["tencent", "wanka"]) + '"'
lines = [line.replace(legacy_host, '"43.155.235.227"') for line in lines]

path.write_text("\n".join(lines) + "\n")
PY
}

install_systemd() {
  local source_dir
  source_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd -P)"
  install -m 0644 -o root -g root "${source_dir}/systemd.service" "${SYSTEMD_UNIT}"
  systemctl daemon-reload
  if [[ "${ENABLE_SERVICE}" -eq 1 ]]; then
    systemctl enable "${SERVICE_NAME}"
    systemctl restart "${SERVICE_NAME}"
  fi
}

install_nginx() {
  [[ "${INSTALL_NGINX}" -eq 1 ]] || return 0
  require_command nginx
  local source_dir snippet target
  source_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd -P)"
  snippet="$(mktemp)"
  sed "s#/nexushub/#${PATH_PREFIX}#g" "${source_dir}/nginx-location.conf" > "${snippet}"
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
  require_root
  require_command tar
  install_dirs
  install_payload
  install_codex_wrappers
  install_config
  install_systemd
  install_nginx
  log "installed ${INSTALL_BIN}"
}

main "$@"
