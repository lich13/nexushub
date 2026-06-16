#!/usr/bin/env bash
set -Eeuo pipefail

IFS=$'\n\t'

APP_NAME="nexushub"
ASSET_NAME="nexushub-linux-x86_64.tar.gz"
REPO="lich13/nexushub"
VERSION="latest"
ARCHIVE_PATH=""
SHA256_PATH=""
KEEP_BACKUPS=3

INSTALL_DIR="/opt/${APP_NAME}"
INSTALL_BIN="${INSTALL_DIR}/bin/nexushubd"
WEBUI_DIR="${INSTALL_DIR}/webui"
BACKUP_DIR="${INSTALL_DIR}/backups/release-updates"
SERVICE_NAME="${APP_NAME}"
CONFIG_FILE="${NEXUSHUB_CONFIG:-${INSTALL_DIR}/config.toml}"
UPDATE_BIN="/usr/local/bin/${APP_NAME}-update"
CODEX_PRECHECK_WRAPPER_BIN="/usr/local/bin/${APP_NAME}-codex-precheck"
CODEX_UPDATE_WRAPPER_BIN="/usr/local/bin/${APP_NAME}-codex-update"
CODEX_PRUNE_WRAPPER_BIN="/usr/local/bin/${APP_NAME}-codex-prune"
HEALTH_URL="http://127.0.0.1:15742/healthz"
GITHUB_TOKEN="${NEXUSHUB_GITHUB_TOKEN:-${GITHUB_TOKEN:-}}"
CURL="${NEXUSHUB_CURL:-curl}"
TAR="${NEXUSHUB_TAR:-tar}"
SHA256SUM="${NEXUSHUB_SHA256SUM:-sha256sum}"
GIT="${NEXUSHUB_GIT:-git}"

usage() {
  cat <<'USAGE'
Update NexusHub.

Usage:
  sudo nexushub-update --repo lich13/nexushub --version latest
  sudo nexushub-update --archive ./nexushub-linux-x86_64.tar.gz --sha256 ./nexushub-linux-x86_64.tar.gz.sha256
USAGE
}

log() { printf '[%s-update] %s\n' "${APP_NAME}" "$*"; }
die() { printf '[%s-update] ERROR: %s\n' "${APP_NAME}" "$*" >&2; exit 1; }

curl_args() {
  printf '%s\n' -fsSL --retry 5 --retry-delay 2 --connect-timeout 20 --max-time 300
  if curl_supports_retry_all_errors; then
    printf '%s\n' --retry-all-errors
  fi
  printf '%s\n' -H "User-Agent: ${APP_NAME}-update"
  if [[ -n "${GITHUB_TOKEN}" ]]; then
    printf '%s\n' -H "Authorization: Bearer ${GITHUB_TOKEN}"
    printf '%s\n' -H "X-GitHub-Api-Version: 2022-11-28"
  fi
}

curl_supports_retry_all_errors() {
  "${CURL}" --help all 2>/dev/null | grep -q -- '--retry-all-errors'
}

download_file() {
  local url="$1"
  local output="$2"
  local accept="${3:-}"
  local args=()

  while IFS= read -r arg; do
    args+=("${arg}")
  done < <(curl_args)
  if [[ -n "${accept}" ]]; then
    args+=(-H "Accept: ${accept}")
  fi

  "${CURL}" "${args[@]}" -o "${output}" "${url}"
}

parse_release_metadata() {
  local json_path="$1"
  local output_path="$2"

  python3 - "${json_path}" "${ASSET_NAME}" > "${output_path}" <<'PY'
import json
import sys

json_path, asset_name = sys.argv[1], sys.argv[2]
with open(json_path, "r", encoding="utf-8") as handle:
    data = json.load(handle)

assets = data.get("assets") or []

def find_asset(name):
    for asset in assets:
        if asset.get("name") == name:
            return asset
    return {}

archive = find_asset(asset_name)
checksum = find_asset(asset_name + ".sha256")

def value(mapping, key):
    return str(mapping.get(key) or "")

print(f"tag_name={data.get('tag_name') or ''}")
print(f"archive_api_url={value(archive, 'url')}")
print(f"archive_browser_url={value(archive, 'browser_download_url')}")
print(f"sha_api_url={value(checksum, 'url')}")
print(f"sha_browser_url={value(checksum, 'browser_download_url')}")
PY
}

load_release_metadata() {
  local release_json="${TMP}/release.json"
  local parsed="${TMP}/release.env"
  local api_url

  if [[ "${VERSION}" == "latest" ]]; then
    api_url="https://api.github.com/repos/${REPO}/releases/latest"
  else
    api_url="https://api.github.com/repos/${REPO}/releases/tags/${VERSION}"
  fi

  log "checking GitHub release metadata: ${api_url}"
  if ! download_file "${api_url}" "${release_json}" "application/vnd.github+json"; then
    log "metadata lookup failed; falling back to direct release URLs"
    return 1
  fi
  if ! parse_release_metadata "${release_json}" "${parsed}"; then
    log "metadata parse failed; falling back to direct release URLs"
    return 1
  fi

  RELEASE_TAG=""
  ARCHIVE_API_URL=""
  ARCHIVE_BROWSER_URL=""
  SHA_API_URL=""
  SHA_BROWSER_URL=""
  while IFS='=' read -r key value; do
    case "${key}" in
      tag_name) RELEASE_TAG="${value}" ;;
      archive_api_url) ARCHIVE_API_URL="${value}" ;;
      archive_browser_url) ARCHIVE_BROWSER_URL="${value}" ;;
      sha_api_url) SHA_API_URL="${value}" ;;
      sha_browser_url) SHA_BROWSER_URL="${value}" ;;
    esac
  done < "${parsed}"

  if [[ -n "${RELEASE_TAG}" ]]; then
    log "resolved release tag ${RELEASE_TAG}"
  fi
}

repo_git_url() {
  case "${REPO}" in
    http://*|https://*|git@*) printf '%s\n' "${REPO}" ;;
    *.git) printf 'https://github.com/%s\n' "${REPO}" ;;
    *) printf 'https://github.com/%s.git\n' "${REPO}" ;;
  esac
}

resolve_latest_tag_from_git_refs() {
  [[ "${VERSION}" == "latest" ]] || return 1
  command -v "${GIT}" >/dev/null || return 1

  local refs="${TMP}/git-tags.txt"
  local tag_file="${TMP}/latest-tag.txt"
  local url

  url="$(repo_git_url)"
  log "resolving latest tag from git refs: ${url}"
  if ! "${GIT}" ls-remote --tags --refs "${url}" 'v*' > "${refs}"; then
    log "git tag lookup failed; falling back to direct latest URL"
    return 1
  fi

  if ! python3 - "${refs}" > "${tag_file}" <<'PY'
import re
import sys

refs_path = sys.argv[1]
pattern = re.compile(r"refs/tags/(v?(\d+)\.(\d+)\.(\d+)(?:[-+][0-9A-Za-z.-]+)?)$")
stable = []
all_tags = []

with open(refs_path, "r", encoding="utf-8") as handle:
    for line in handle:
        match = pattern.search(line.strip())
        if not match:
            continue
        tag = match.group(1)
        major = int(match.group(2))
        minor = int(match.group(3))
        patch = int(match.group(4))
        entry = ((major, minor, patch), tag)
        all_tags.append(entry)
        if "-" not in tag:
            stable.append(entry)

choices = stable or all_tags
if not choices:
    raise SystemExit(1)

print(max(choices, key=lambda item: item[0])[1])
PY
  then
    log "git tag parse failed; falling back to direct latest URL"
    return 1
  fi

  RELEASE_TAG="$(tr -d '\r\n' < "${tag_file}")"
  [[ -n "${RELEASE_TAG}" ]] || return 1
  log "resolved release tag ${RELEASE_TAG} from git refs"
}

try_download_pair() {
  local label="$1"
  local archive_url="$2"
  local sha_url="$3"
  local accept="${4:-}"
  local archive_tmp="${TMP}/${ASSET_NAME}.${label}.tmp"
  local sha_tmp="${TMP}/${ASSET_NAME}.${label}.sha256.tmp"

  [[ -n "${archive_url}" && -n "${sha_url}" ]] || return 1

  log "downloading archive via ${label}: ${archive_url}"
  rm -f "${archive_tmp}" "${sha_tmp}"
  if download_file "${archive_url}" "${archive_tmp}" "${accept}" && download_file "${sha_url}" "${sha_tmp}" "${accept}"; then
    mv "${archive_tmp}" "${ARCHIVE_PATH}"
    mv "${sha_tmp}" "${SHA256_PATH}"
    log "downloaded release assets via ${label}"
    return 0
  fi

  rm -f "${archive_tmp}" "${sha_tmp}"
  log "download via ${label} failed; trying next source"
  return 1
}

download_release() {
  local tag=""
  local tag_base=""
  local latest_base="https://github.com/${REPO}/releases/latest/download"

  ARCHIVE_PATH="${TMP}/${ASSET_NAME}"
  SHA256_PATH="${TMP}/${ASSET_NAME}.sha256"

  RELEASE_TAG=""
  ARCHIVE_API_URL=""
  ARCHIVE_BROWSER_URL=""
  SHA_API_URL=""
  SHA_BROWSER_URL=""
  load_release_metadata || true
  if [[ -z "${RELEASE_TAG}" && "${VERSION}" == "latest" ]]; then
    resolve_latest_tag_from_git_refs || true
  fi

  tag="${RELEASE_TAG}"
  if [[ -z "${tag}" && "${VERSION}" != "latest" ]]; then
    tag="${VERSION}"
  fi
  if [[ -n "${tag}" ]]; then
    tag_base="https://github.com/${REPO}/releases/download/${tag}"
  fi

  try_download_pair "api" "${ARCHIVE_API_URL}" "${SHA_API_URL}" "application/octet-stream" && return 0
  try_download_pair "browser" "${ARCHIVE_BROWSER_URL}" "${SHA_BROWSER_URL}" && return 0
  if [[ -n "${tag_base}" ]]; then
    try_download_pair "tag" "${tag_base}/${ASSET_NAME}" "${tag_base}/${ASSET_NAME}.sha256" && return 0
  fi
  if [[ "${VERSION}" == "latest" ]]; then
    try_download_pair "latest" "${latest_base}/${ASSET_NAME}" "${latest_base}/${ASSET_NAME}.sha256" && return 0
  fi

  die "could not download ${ASSET_NAME} and checksum for ${REPO}@${VERSION}"
}

migrate_codex_update_config() {
  local config_file="${1:-${CONFIG_FILE}}"
  [[ -f "${config_file}" ]] || return 0

  python3 - "${config_file}" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
lines = path.read_text().splitlines()

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

replacements = {
    "update_command": {
        '"sudo -n /home/ubuntu/codex-admin/bin/codex-cloud-update --no-prune"',
        '"/home/ubuntu/codex-admin/bin/codex-cloud-update --no-prune"',
    },
    "prune_command": {
        '"sudo -n /home/ubuntu/codex-admin/bin/codex-cloud-prune"',
        '"/home/ubuntu/codex-admin/bin/codex-cloud-prune"',
    },
}
new_values = {
    "precheck_command": '"/usr/local/bin/nexushub-codex-precheck"',
    "update_command": '"/usr/local/bin/nexushub-codex-update"',
    "prune_command": '"/usr/local/bin/nexushub-codex-prune"',
    "panel_update_command": '"/usr/local/bin/nexushub-update --repo lich13/nexushub --version latest"',
    "panel_precheck_command": '"test -x /usr/local/bin/nexushub-update && systemctl is-active nexushub && curl -fsS http://127.0.0.1:15742/healthz"',
}

changed = False

def ensure_section(section, required_values, replace_values=None, insert_if_missing=True):
    global changed
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
        if not insert_if_missing:
            return
        if lines and lines[-1].strip():
            lines.append("")
        lines.append(f"[{section}]")
        start = len(lines) - 1
        end = len(lines)
        changed = True

    seen = set()
    for index in range(start + 1, end):
        stripped = lines[index].strip()
        if not stripped or stripped.startswith("#") or "=" not in stripped:
            continue
        key, value = [part.strip() for part in stripped.split("=", 1)]
        if (key in replace_values and value in replace_values[key]) or is_legacy_codex_precheck(key, value):
            lines[index] = f"{key} = {required_values[key]}"
            changed = True
        if key in required_values:
            seen.add(key)

    insert_at = end
    for key, value in required_values.items():
        if key not in seen:
            lines.insert(insert_at, f"{key} = {value}")
            insert_at += 1
            changed = True

def remove_section_keys(section, keys):
    global changed, lines
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
    removed = False
    for index, line in enumerate(lines):
        if start < index < end:
            stripped = line.strip()
            if stripped and not stripped.startswith("#") and "=" in stripped:
                key = stripped.split("=", 1)[0].strip()
                if key in keys:
                    removed = True
                    continue
        filtered.append(line)
    if removed:
        lines = filtered
        changed = True

ensure_section(
    "codex",
    {
        "host_label": '"43.155.235.227"',
    },
)
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
ensure_section(
    "server",
    {"listen": '"127.0.0.1:15742"'},
    {"listen": {'"127.0.0.1:15732"'}},
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
    new_values,
    {
        **replacements,
        "panel_precheck_command": {
            '"test -x /usr/local/bin/nexushub-update && systemctl is-active nexushub && curl -fsS http://127.0.0.1:15732/healthz"',
        },
    },
)

ensure_section(
    "probe",
    {
        "enabled": "true",
        "poll_seconds": "15",
        "recent_limit": "50",
    },
)
ensure_section(
    "probe.hooks",
    {
        "manage_stop_hook": "true",
    },
)
remove_section_keys(
    "probe.hooks",
    {
        "reload_app_server_after_install",
    },
)
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
    {
        "hook_event_max_lines": "500",
        "hook_cooldown_max_lines": "1000",
        "log_max_bytes": "5242880",
    },
    {
        "hook_event_max_lines": {"120"},
        "hook_cooldown_max_lines": {"80"},
        "log_max_bytes": {"262144"},
    },
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

if changed:
    path.write_text("\n".join(lines) + "\n")
PY
}

main() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --repo) REPO="${2:-}"; shift 2 ;;
      --version) VERSION="${2:-}"; shift 2 ;;
      --archive) ARCHIVE_PATH="${2:-}"; shift 2 ;;
      --sha256) SHA256_PATH="${2:-}"; shift 2 ;;
      --keep-backups) KEEP_BACKUPS="${2:-}"; shift 2 ;;
      -h|--help) usage; exit 0 ;;
      *) die "unknown argument: $1" ;;
    esac
  done

  [[ "${EUID}" -eq 0 ]] || die "run as root"
  command -v "${CURL}" >/dev/null || die "curl missing"
  command -v "${TAR}" >/dev/null || die "tar missing"
  command -v "${SHA256SUM}" >/dev/null || die "sha256sum missing"
  if [[ -z "${ARCHIVE_PATH}" ]]; then
    command -v python3 >/dev/null || die "python3 missing"
  fi

  TMP="$(mktemp -d)"
  trap 'rm -rf "${TMP}"' EXIT

  if [[ -z "${ARCHIVE_PATH}" ]]; then
    download_release
  fi

  if [[ -n "${SHA256_PATH}" && -f "${SHA256_PATH}" ]]; then
    expected="$(awk '{print $1; exit}' "${SHA256_PATH}")"
    actual="$("${SHA256SUM}" "${ARCHIVE_PATH}" | awk '{print $1}')"
    [[ "${expected}" == "${actual}" ]] || die "sha256 mismatch"
  fi

  PAYLOAD="${TMP}/payload"
  mkdir -p "${PAYLOAD}"
  "${TAR}" -xzf "${ARCHIVE_PATH}" -C "${PAYLOAD}"
  ROOT="${PAYLOAD}/${APP_NAME}"
  [[ -x "${ROOT}/bin/nexushubd" ]] || die "archive missing binary"

  STAMP="$(date +%Y%m%d-%H%M%S)"
  BACKUP="${BACKUP_DIR}/${STAMP}"
  mkdir -p "${BACKUP}"
  [[ -f "${INSTALL_BIN}" ]] && cp "${INSTALL_BIN}" "${BACKUP}/nexushubd"
  [[ -d "${WEBUI_DIR}" ]] && cp -a "${WEBUI_DIR}" "${BACKUP}/webui"

  install -m 0755 -o root -g root "${ROOT}/bin/nexushubd" "${INSTALL_BIN}"
  rm -rf "${WEBUI_DIR}"
  install -d -m 0755 -o root -g root "${WEBUI_DIR}"
  cp -a "${ROOT}/webui/." "${WEBUI_DIR}/"
  chown -R root:root "${WEBUI_DIR}"
  if [[ -d "${ROOT}/deploy" ]]; then
    [[ -f "${ROOT}/deploy/update.sh" ]] && install -m 0755 -o root -g root "${ROOT}/deploy/update.sh" "${UPDATE_BIN}"
    [[ -f "${ROOT}/deploy/${APP_NAME}-codex-precheck" ]] && install -m 0755 -o root -g root "${ROOT}/deploy/${APP_NAME}-codex-precheck" "${CODEX_PRECHECK_WRAPPER_BIN}"
    [[ -f "${ROOT}/deploy/${APP_NAME}-codex-update" ]] && install -m 0755 -o root -g root "${ROOT}/deploy/${APP_NAME}-codex-update" "${CODEX_UPDATE_WRAPPER_BIN}"
    [[ -f "${ROOT}/deploy/${APP_NAME}-codex-prune" ]] && install -m 0755 -o root -g root "${ROOT}/deploy/${APP_NAME}-codex-prune" "${CODEX_PRUNE_WRAPPER_BIN}"
  fi
  migrate_codex_update_config

  systemctl restart "${SERVICE_NAME}"
  for _ in $(seq 1 30); do
    if "${CURL}" -fsS "${HEALTH_URL}" >/dev/null 2>&1; then
      log "health ok"
      find "${BACKUP_DIR}" -maxdepth 1 -mindepth 1 -type d | sort | head -n -"${KEEP_BACKUPS}" | xargs -r rm -rf
      exit 0
    fi
    sleep 1
  done

  log "health failed, rolling back"
  [[ -f "${BACKUP}/nexushubd" ]] && install -m 0755 -o root -g root "${BACKUP}/nexushubd" "${INSTALL_BIN}"
  if [[ -d "${BACKUP}/webui" ]]; then
    rm -rf "${WEBUI_DIR}"
    cp -a "${BACKUP}/webui" "${WEBUI_DIR}"
  fi
  systemctl restart "${SERVICE_NAME}" || true
  die "update failed"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
