#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd -P)"
INSTALL_SH="${ROOT}/deploy/nexushub/install.sh"
UPDATE_SH="${ROOT}/deploy/nexushub/update.sh"
DEPLOY_CLOUD_SH="${ROOT}/scripts/deploy-cloud.sh"
CONFIG_EXAMPLE="${ROOT}/deploy/nexushub/config.example.toml"
NGINX_LOCATION="${ROOT}/deploy/nexushub/nginx-location.conf"
SYSTEMD_SERVICE="${ROOT}/deploy/nexushub/systemd.service"
CODEX_PRECHECK_WRAPPER="${ROOT}/deploy/nexushub/nexushub-codex-precheck"
CODEX_UPDATE_WRAPPER="${ROOT}/deploy/nexushub/nexushub-codex-update"
CODEX_PRUNE_WRAPPER="${ROOT}/deploy/nexushub/nexushub-codex-prune"

python3 - "${CONFIG_EXAMPLE}" "${NGINX_LOCATION}" "${INSTALL_SH}" "${UPDATE_SH}" "${DEPLOY_CLOUD_SH}" <<'PY'
from pathlib import Path
import sys

config, nginx, install, update, deploy = [Path(arg).read_text() for arg in sys.argv[1:]]

checks = {
    "config.example server listen": (config, 'listen = "127.0.0.1:15742"'),
    "config.example panel precheck": (config, "http://127.0.0.1:15742/healthz"),
    "config.example probe section": (config, "[probe]\nenabled = true\npoll_seconds = 15\nrecent_limit = 50"),
    "config.example probe hooks": (config, "[probe.hooks]\nmanage_stop_hook = true\nreload_app_server_after_install = true"),
    "config.example probe notifications": (config, "[probe.notifications]\nenabled = false\nserver_url = \"https://api.day.app\""),
    "config.example probe observability": (config, "[probe.observability]\nhook_event_max_lines = 500\nhook_cooldown_max_lines = 1000\nlog_max_bytes = 5242880"),
    "config.example probe logs db": (config, "[probe.logs_db]\nenabled = true\nretention_days = 2\nmaintenance_interval_hours = 6"),
    "nginx proxy target": (nginx, "proxy_pass http://127.0.0.1:15742/;"),
    "install config migration": (install, "http://127.0.0.1:15742/healthz"),
    "install legacy listen migration": (install, '"listen": \'"127.0.0.1:15742"\''),
    "update health URL": (update, 'HEALTH_URL="http://127.0.0.1:15742/healthz"'),
    "deploy smoke health URL": (deploy, "http://127.0.0.1:15742/healthz"),
}

missing = [name for name, (text, needle) in checks.items() if needle not in text]
if missing:
    raise SystemExit("NexusHub deploy templates must use a port distinct from codex-cloud-panel: " + ", ".join(missing))

for name, text in {
    "config.example": config,
    "nginx-location.conf": nginx,
    "deploy-cloud.sh": deploy,
}.items():
    if "127.0.0.1:15732" in text:
        raise SystemExit(f"{name} must not point NexusHub at legacy codex-cloud-panel port 15732")

print("NexusHub deploy port isolation: ok")
PY

python3 - "${CONFIG_EXAMPLE}" "${SYSTEMD_SERVICE}" <<'PY'
from pathlib import Path
import re
import sys

config, systemd = [Path(arg).read_text() for arg in sys.argv[1:]]

codex_match = re.search(r"(?ms)^\[codex\]\n(?P<body>.*?)(?=^\[|\Z)", config)
if not codex_match:
    raise SystemExit("config.example must include a [codex] section")
codex_body = codex_match.group("body")
if re.search(r"(?m)^\s*home\s*=", codex_body):
    raise SystemExit("config.example must omit codex.home so runtime auto-discovery applies")
if 'app_server_socket = "/root/.codex/app-server-control/app-server-control.sock"' not in codex_body:
    raise SystemExit("config.example should keep the default root app-server socket")

rw_match = re.search(r"(?m)^ReadWritePaths=(?P<paths>.+)$", systemd)
if not rw_match:
    raise SystemExit("systemd unit must define ReadWritePaths")
paths = rw_match.group("paths").split()
required = {"/root/.codex", "/home/ubuntu/.codex", "/opt/nexushub", "/opt/nexushub/logs"}
missing = sorted(required - set(paths))
if missing:
    raise SystemExit("systemd ReadWritePaths missing resolved Codex/NexusHub paths: " + ", ".join(missing))
codex_paths = [path for path in paths if path.endswith("/.codex")]
if codex_paths != ["/root/.codex", "/home/ubuntu/.codex"]:
    raise SystemExit("systemd ReadWritePaths should only grant known root/ubuntu Codex homes, got: " + " ".join(codex_paths))

print("Codex home auto-discovery deploy templates: ok")
PY

python3 - "${INSTALL_SH}" <<'PY'
from pathlib import Path
import re
import sys

path = Path(sys.argv[1])
text = path.read_text()
codex_paths_match = re.search(r"install_codex_home_write_paths\(\) \{\n(?P<body>.*?)\n\}", text, re.S)
if not codex_paths_match:
    raise SystemExit("install_codex_home_write_paths function not found")

codex_paths_body = codex_paths_match.group("body")
for needle in [
    "/root/.codex",
    "/home/ubuntu/.codex",
    "getent passwd ubuntu",
    "install -d -m 0700",
]:
    if needle not in codex_paths_body:
        raise SystemExit(f"install_codex_home_write_paths missing {needle}")

match = re.search(r"install_systemd\(\) \{\n(?P<body>.*?)\n\}", text, re.S)
if not match:
    raise SystemExit("install_systemd function not found")

body = match.group("body")
if 'systemctl restart "${SERVICE_NAME}"' not in body:
    raise SystemExit("install_systemd must restart an already-installed service after replacing the binary")

if body.find('systemctl restart "${SERVICE_NAME}"') < body.find('systemctl daemon-reload'):
    raise SystemExit("install_systemd must restart only after daemon-reload")

main_match = re.search(r"main\(\) \{\n(?P<body>.*?)\n\}", text, re.S)
if not main_match:
    raise SystemExit("main function not found")
main_body = main_match.group("body")
if 'install_codex_home_write_paths' not in main_body:
    raise SystemExit("main must prepare Codex home write paths before installing systemd")
if main_body.find('install_codex_home_write_paths') > main_body.find('install_systemd'):
    raise SystemExit("main must prepare Codex home write paths before install_systemd")

print("install_systemd restart behavior: ok")
PY

python3 - "${INSTALL_SH}" <<'PY'
from pathlib import Path
import itertools
import re
import shlex
import subprocess
import sys
import tempfile

install_path = Path(sys.argv[1])
text = install_path.read_text()
match = re.search(r"ensure_secret_key\(\) \{\n(?P<body>.*?)\n\}", text, re.S)
if not match:
    raise SystemExit("ensure_secret_key function not found")

function_text = match.group(0)

with tempfile.TemporaryDirectory() as tmp:
    tmp_path = Path(tmp)
    panel_env = tmp_path / "codex-cloud-panel.env"
    cc_env = tmp_path / "cc-switch-lite.env"
    counter = itertools.count()

    patched_function = (
        function_text
        .replace("/etc/codex-cloud-panel/env", str(panel_env))
        .replace("/etc/cc-switch-lite/env", str(cc_env))
        .replace('chown root:root "${ENV_FILE}"', ":")
    )

    def write_optional(path: Path, content: str | None) -> None:
        if path.exists():
            path.unlink()
        if content is not None:
            path.write_text(content)

    def run_case(env_text: str, panel_secret: str | None, cc_secret: str | None) -> str:
        env_file = tmp_path / f"env-{next(counter)}"
        env_file.write_text(env_text)
        write_optional(
            panel_env,
            None
            if panel_secret is None
            else f'CODEX_CLOUD_PANEL_SECRET_KEY="{panel_secret}"\n',
        )
        write_optional(
            cc_env,
            None
            if cc_secret is None
            else f"CC_SWITCH_LITE_SECRET_KEY='{cc_secret}'\n",
        )
        script = "\n".join(
            [
                "set -Eeuo pipefail",
                f"ENV_FILE={shlex.quote(str(env_file))}",
                patched_function,
                "ensure_secret_key",
            ]
        )
        subprocess.run(["bash", "-c", script], check=True)
        return env_file.read_text()

    preserved = run_case(
        "NEXUSHUB_SECRET_KEY=existing-nexus-secret\n",
        "legacy-panel-secret",
        "cc-switch-secret",
    )
    if preserved.count("NEXUSHUB_SECRET_KEY=") != 1:
        raise SystemExit("existing NEXUSHUB_SECRET_KEY should not be duplicated")
    if "NEXUSHUB_SECRET_KEY=existing-nexus-secret" not in preserved:
        raise SystemExit("existing NEXUSHUB_SECRET_KEY should be preserved")

    imported_panel = run_case("# generated by installer\n", "legacy-panel-secret", "cc-switch-secret")
    if "NEXUSHUB_SECRET_KEY=legacy-panel-secret" not in imported_panel:
        raise SystemExit("CODEX_CLOUD_PANEL_SECRET_KEY should be imported before cc-switch-lite")
    if "NEXUSHUB_SECRET_KEY=cc-switch-secret" in imported_panel:
        raise SystemExit("cc-switch-lite key should not override legacy panel key")

    imported_cc = run_case("", None, "cc-switch-secret")
    if "NEXUSHUB_SECRET_KEY=cc-switch-secret" not in imported_cc:
        raise SystemExit("CC_SWITCH_LITE_SECRET_KEY should be imported when panel key is unavailable")

    generated = run_case("", None, None)
    generated_lines = [
        line for line in generated.splitlines() if line.startswith("NEXUSHUB_SECRET_KEY=")
    ]
    if len(generated_lines) != 1 or generated_lines[0] == "NEXUSHUB_SECRET_KEY=":
        raise SystemExit("installer should generate one non-empty NEXUSHUB_SECRET_KEY when no legacy key exists")

print("install secret key inheritance behavior: ok")
PY

python3 - "${INSTALL_SH}" "${UPDATE_SH}" "${CODEX_PRECHECK_WRAPPER}" "${CODEX_UPDATE_WRAPPER}" "${CODEX_PRUNE_WRAPPER}" <<'PY'
from pathlib import Path
import re
import subprocess
import sys
import tempfile

install_path = Path(sys.argv[1])
update_path = Path(sys.argv[2])
precheck_wrapper = Path(sys.argv[3])
update_wrapper = Path(sys.argv[4])
prune_wrapper = Path(sys.argv[5])
install_text = install_path.read_text()
update_text = update_path.read_text()

for needle in [
    'CONFIG_DIR="${INSTALL_DIR}"',
    'CONFIG_FILE="${CONFIG_DIR}/config.toml"',
    'ENV_FILE="${CONFIG_DIR}/env"',
    'DATA_DIR="${INSTALL_DIR}"',
    'LOG_DIR="${INSTALL_DIR}/logs"',
    'WEBUI_DIR="${INSTALL_DIR}/webui"',
    'nexushub.sqlite',
]:
    if needle not in install_text:
        raise SystemExit(f"install.sh must keep NexusHub Linux runtime under /opt/nexushub: missing {needle}")

for path, expected_commands in [
    (
        precheck_wrapper,
        [
            "sudo -n codex --version",
            "sudo -n codex mcp list",
            "/usr/local/bin/codex-raw --version",
            "/home/ubuntu/codex-admin/bin/codex-cloud-doctor",
        ],
    ),
    (
        update_wrapper,
        [
            "/home/ubuntu/codex-admin/bin/codex-cloud-update --no-prune",
            "/home/ubuntu/codex-admin/bin/codex-cloud-prune",
            "codex --version",
            "codex mcp list",
            "/home/ubuntu/codex-admin/bin/codex-cloud-doctor",
        ],
    ),
    (prune_wrapper, ["/home/ubuntu/codex-admin/bin/codex-cloud-prune"]),
]:
    if not path.exists():
        raise SystemExit(f"missing codex wrapper: {path}")
    text = path.read_text()
    for needle in [
        "systemd-run",
        "--wait",
        "--collect",
        "--pipe",
        "--property=User=root",
        "--property=WorkingDirectory=/home/ubuntu/codex-workspace",
    ]:
        if needle not in text:
            raise SystemExit(f"{path.name} missing {needle}")
    for expected_command in expected_commands:
        if expected_command not in text:
            raise SystemExit(f"{path.name} missing {expected_command}")

for needle in [
    "CODEX_PRECHECK_WRAPPER_BIN",
    "CODEX_UPDATE_WRAPPER_BIN",
    "CODEX_PRUNE_WRAPPER_BIN",
    "nexushub-codex-precheck",
    "nexushub-codex-update",
    "nexushub-codex-prune",
]:
    if needle not in install_text:
        raise SystemExit(f"install.sh missing {needle}")

for name, text in {
    "install.sh": install_text,
    "update.sh": update_text,
}.items():
    for forbidden in [
        "PROBE_LEGACY_CLEANUP",
        "nexushub-probe-legacy-cleanup",
        "probe-legacy-cleanup",
    ]:
        if forbidden in text:
            raise SystemExit(f"{name} must not package one-time legacy cleanup helper: {forbidden}")

legacy_cleanup_path = install_path.parent / "nexushub-probe-legacy-cleanup"
if legacy_cleanup_path.exists():
    raise SystemExit(f"one-time legacy cleanup helper should not be packaged: {legacy_cleanup_path}")

match = re.search(r"python3 - \"\$\{CONFIG_FILE\}\" <<'PY'\n(?P<body>.*?)\nPY", install_text, re.S)
if not match:
    raise SystemExit("install.sh config migration Python block not found")

block = match.group("body")

def run_config_migration(config_text: str) -> str:
    with tempfile.TemporaryDirectory() as tmp:
        config_path = Path(tmp) / "config.toml"
        config_path.write_text(config_text)
        subprocess.run(
            ["python3", "-", str(config_path)],
            input=block,
            text=True,
            check=True,
        )
        return config_path.read_text()

legacy_config = """
[server]
listen = "127.0.0.1:15732"

[update]
update_command = "sudo -n /home/ubuntu/codex-admin/bin/codex-cloud-update --no-prune"
prune_command = "sudo -n /home/ubuntu/codex-admin/bin/codex-cloud-prune"
"""
migrated = run_config_migration(legacy_config)
if 'listen = "127.0.0.1:15742"' not in migrated:
    raise SystemExit("server.listen was not migrated off the legacy codex-cloud-panel port")
if 'data_dir = "/opt/nexushub"' not in migrated:
    raise SystemExit("paths.data_dir was not inserted")
if 'db_path = "/opt/nexushub/nexushub.sqlite"' not in migrated:
    raise SystemExit("paths.db_path was not inserted")
if 'webui_dir = "/opt/nexushub/webui"' not in migrated:
    raise SystemExit("paths.webui_dir was not inserted")
if 'log_dir = "/opt/nexushub/logs"' not in migrated:
    raise SystemExit("paths.log_dir was not inserted")
if 'precheck_command = "/usr/local/bin/nexushub-codex-precheck"' not in migrated:
    raise SystemExit("precheck_command was not inserted")
if 'update_command = "/usr/local/bin/nexushub-codex-update"' not in migrated:
    raise SystemExit("legacy update_command was not migrated to panel codex wrapper")
if 'prune_command = "/usr/local/bin/nexushub-codex-prune"' not in migrated:
    raise SystemExit("legacy prune_command was not migrated to panel codex wrapper")
if 'panel_precheck_command = "test -x /usr/local/bin/nexushub-update && systemctl is-active nexushub && curl -fsS http://127.0.0.1:15742/healthz"' not in migrated:
    raise SystemExit("panel_precheck_command was not inserted with the isolated NexusHub port")
if re.search(r"(?m)^\s*home\s*=", migrated):
    raise SystemExit("install migration should not insert codex.home; runtime auto-discovery should apply")
for needle in [
    "[probe]\nenabled = true\npoll_seconds = 15\nrecent_limit = 50",
    "[probe.hooks]\nmanage_stop_hook = true\nreload_app_server_after_install = true",
    "[probe.notifications]\nenabled = false\nserver_url = \"https://api.day.app\"",
    "notify_completion = true",
    "notify_reply_needed = true",
    "notify_recoverable = true",
    "[probe.observability]\nhook_event_max_lines = 500\nhook_cooldown_max_lines = 1000\nlog_max_bytes = 5242880",
    "[probe.logs_db]\nenabled = true\nretention_days = 2\nmaintenance_interval_hours = 6",
    "maintain_on_codex_exit = true",
    "codex_exit_grace_seconds = 5",
    "codex_exit_max_wait_seconds = 1800",
    "delete_chunk_rows = 5000",
    "max_delete_rows_per_run = 100000",
    "busy_timeout_ms = 500",
    "auto_compact_when_codex_closed = true",
    "compact_interval_hours = 24",
    "compact_min_freelist_mb = 256",
    "compact_min_freelist_ratio_percent = 20",
    "minimum_free_space_mb = 1024",
]:
    if needle not in migrated:
        raise SystemExit(f"probe default was not inserted by install migration: {needle}")

legacy_precheck_config = '''
[update]
precheck_command = "codex --version && sudo -n codex --version && /usr/local/bin/codex-raw --version && readlink -f /usr/local/bin/codex && readlink -f /usr/local/bin/codex-raw && sqlite3 /root/.codex/state_5.sqlite 'pragma integrity_check;' && sqlite3 /root/.codex/state_5.sqlite \\"select count(*) total, sum(archived_at is null) active, sum(archived_at is not null) archived from threads;\\" && /home/ubuntu/codex-admin/bin/codex-cloud-doctor"
update_command = "/usr/local/bin/nexushub-codex-update"
prune_command = "/usr/local/bin/nexushub-codex-prune"
'''
migrated_precheck = run_config_migration(legacy_precheck_config)
if 'precheck_command = "/usr/local/bin/nexushub-codex-precheck"' not in migrated_precheck:
    raise SystemExit("legacy precheck_command was not migrated to panel codex wrapper")

custom_config = """
[paths]
data_dir = "/srv/custom/nexushub"
db_path = "/srv/custom/nexushub/custom.sqlite"
webui_dir = "/srv/custom/nexushub/webui"
log_dir = "/srv/custom/nexushub/logs"

[update]
update_command = "/opt/custom/update --flag"
prune_command = "/opt/custom/prune"
"""
migrated_custom = run_config_migration(custom_config)
if 'data_dir = "/srv/custom/nexushub"' not in migrated_custom:
    raise SystemExit("custom paths.data_dir should not be overwritten")
if 'db_path = "/srv/custom/nexushub/custom.sqlite"' not in migrated_custom:
    raise SystemExit("custom paths.db_path should not be overwritten")
if 'update_command = "/opt/custom/update --flag"' not in migrated_custom:
    raise SystemExit("custom update_command should not be overwritten")
if 'prune_command = "/opt/custom/prune"' not in migrated_custom:
    raise SystemExit("custom prune_command should not be overwritten")
custom_codex_config = """
[codex]
home = "/srv/codex/custom-home"
app_server_socket = "/srv/codex/custom-home/app-server-control/app-server-control.sock"
"""
migrated_custom_codex = run_config_migration(custom_codex_config)
if 'home = "/srv/codex/custom-home"' not in migrated_custom_codex:
    raise SystemExit("custom codex.home should be preserved by install migration")
if 'app_server_socket = "/srv/codex/custom-home/app-server-control/app-server-control.sock"' not in migrated_custom_codex:
    raise SystemExit("custom codex app-server socket should be preserved by install migration")
for needle in [
    'enabled = false',
    'poll_seconds = 45',
    'recent_limit = 12',
    'manage_stop_hook = false',
    'server_url = "https://bark.example"',
    'group = "Custom Ops"',
    'log_max_bytes = 65536',
    'retention_days = 90',
    'minimum_free_space_mb = 1024',
]:
    custom_probe = """
[probe]
enabled = false
poll_seconds = 45
recent_limit = 12

[probe.hooks]
manage_stop_hook = false

[probe.notifications]
server_url = "https://bark.example"
group = "Custom Ops"

[probe.observability]
log_max_bytes = 65536

[probe.logs_db]
retention_days = 90
minimum_free_space_mb = 1024
"""
    migrated_probe = run_config_migration(custom_probe)
    if needle not in migrated_probe:
        raise SystemExit(f"custom probe value should be preserved by install migration: {needle}")
if 'reload_app_server_after_install = true' not in migrated_probe:
    raise SystemExit("install migration should fill missing probe.hooks defaults")
if 'notify_recoverable = true' not in migrated_probe:
    raise SystemExit("install migration should fill missing probe.notifications defaults")
if 'busy_timeout_ms = 500' not in migrated_probe:
    raise SystemExit("install migration should fill missing probe.logs_db defaults")

legacy_probe_defaults = """
[probe.observability]
hook_event_max_lines = 120
hook_cooldown_max_lines = 80
log_max_bytes = 262144

[probe.logs_db]
enabled = true
retention_days = 14
maintenance_interval_hours = 24
maintain_on_codex_exit = true
codex_exit_grace_seconds = 10
codex_exit_max_wait_seconds = 120
delete_chunk_rows = 2000
max_delete_rows_per_run = 50000
busy_timeout_ms = 5000
auto_compact_when_codex_closed = true
compact_interval_hours = 168
compact_min_freelist_mb = 64
compact_min_freelist_ratio_percent = 20
minimum_free_space_mb = 256
"""
migrated_probe_defaults = run_config_migration(legacy_probe_defaults)
for needle in [
    "hook_event_max_lines = 500",
    "hook_cooldown_max_lines = 1000",
    "log_max_bytes = 5242880",
    "retention_days = 2",
    "maintenance_interval_hours = 6",
    "codex_exit_grace_seconds = 5",
    "codex_exit_max_wait_seconds = 1800",
    "delete_chunk_rows = 5000",
    "max_delete_rows_per_run = 100000",
    "busy_timeout_ms = 500",
    "compact_interval_hours = 24",
    "compact_min_freelist_mb = 256",
    "minimum_free_space_mb = 1024",
]:
    if needle not in migrated_probe_defaults:
        raise SystemExit(f"old probe default was not migrated by install migration: {needle}")
for stale in [
    "hook_event_max_lines = 120",
    "hook_cooldown_max_lines = 80",
    "log_max_bytes = 262144",
    "retention_days = 14",
    "maintenance_interval_hours = 24",
    "codex_exit_grace_seconds = 10",
    "codex_exit_max_wait_seconds = 120",
    "delete_chunk_rows = 2000",
    "max_delete_rows_per_run = 50000",
    "busy_timeout_ms = 5000",
    "compact_interval_hours = 168",
    "compact_min_freelist_mb = 64",
    "minimum_free_space_mb = 256",
]:
    if stale in migrated_probe_defaults:
        raise SystemExit(f"old probe default should be replaced by install migration: {stale}")

legacy_paths_config = """
[paths]
data_dir = "/var/lib/codex-cloud-panel"
db_path = "/var/lib/codex-cloud-panel/panel.sqlite"
webui_dir = "/usr/share/codex-cloud-panel/webui"
log_dir = "/var/log/codex-cloud-panel"
"""
migrated_paths = run_config_migration(legacy_paths_config)
if 'data_dir = "/opt/nexushub"' not in migrated_paths:
    raise SystemExit("legacy paths.data_dir was not migrated")
if 'db_path = "/opt/nexushub/nexushub.sqlite"' not in migrated_paths:
    raise SystemExit("legacy paths.db_path was not migrated")
if 'webui_dir = "/opt/nexushub/webui"' not in migrated_paths:
    raise SystemExit("legacy paths.webui_dir was not migrated")
if 'log_dir = "/opt/nexushub/logs"' not in migrated_paths:
    raise SystemExit("legacy paths.log_dir was not migrated")

print("codex update/prune wrapper install and migration behavior: ok")
PY

bash -n "${UPDATE_SH}"

python3 - "${UPDATE_SH}" <<'PY'
from pathlib import Path
import sys

text = Path(sys.argv[1]).read_text()

required = {
    "GitHub latest release API": "/releases/latest",
    "GitHub release-by-tag API": "/releases/tags/",
    "release asset API URL": "application/octet-stream",
    "browser asset URL fallback": "browser_download_url",
    "git tag fallback": "ls-remote --tags --refs",
    "stable tag resolver": "resolve_latest_tag_from_git_refs",
    "tagged release fallback": "/releases/download/${tag}",
    "latest download fallback": "/releases/latest/download",
    "retry transient HTTP failures": "--retry-all-errors",
    "connect timeout": "--connect-timeout",
    "overall timeout": "--max-time",
    "self update installs precheck wrapper": "CODEX_PRECHECK_WRAPPER_BIN",
    "self update installs update wrapper": "CODEX_UPDATE_WRAPPER_BIN",
    "self update installs prune wrapper": "CODEX_PRUNE_WRAPPER_BIN",
    "self update copies deploy precheck wrapper": "${ROOT}/deploy/${APP_NAME}-codex-precheck",
    "self update copies deploy wrappers": "${ROOT}/deploy/${APP_NAME}-codex-update",
    "self update migrates old codex config": "migrate_codex_update_config",
}

missing = [name for name, needle in required.items() if needle not in text]
if missing:
    raise SystemExit("update.sh missing resilient download behavior: " + ", ".join(missing))

print("update release download fallback behavior: ok")
PY

python3 - "${UPDATE_SH}" <<'PY'
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import textwrap

update_sh = Path(sys.argv[1])

with tempfile.TemporaryDirectory() as tmp:
    config = Path(tmp) / "config.toml"
    config.write_text(textwrap.dedent("""
        [server]
        listen = "127.0.0.1:15732"

        [update]
        update_command = "sudo -n /home/ubuntu/codex-admin/bin/codex-cloud-update --no-prune"
        prune_command = "sudo -n /home/ubuntu/codex-admin/bin/codex-cloud-prune"
    """).lstrip())
    subprocess.run(
        ["bash", "-c", f"source {update_sh}; migrate_codex_update_config {config}"],
        check=True,
    )
    migrated = config.read_text()
    if 'listen = "127.0.0.1:15742"' not in migrated:
        raise SystemExit("update.sh did not migrate server.listen off the legacy codex-cloud-panel port")
    if 'precheck_command = "/usr/local/bin/nexushub-codex-precheck"' not in migrated:
        raise SystemExit("update.sh did not insert codex precheck command")
    if 'update_command = "/usr/local/bin/nexushub-codex-update"' not in migrated:
        raise SystemExit("update.sh did not migrate legacy codex update command")
    if 'prune_command = "/usr/local/bin/nexushub-codex-prune"' not in migrated:
        raise SystemExit("update.sh did not migrate legacy codex prune command")
    if 'db_path = "/opt/nexushub/nexushub.sqlite"' not in migrated:
        raise SystemExit("update.sh did not insert NexusHub DB path")
    if 'panel_precheck_command = "test -x /usr/local/bin/nexushub-update && systemctl is-active nexushub && curl -fsS http://127.0.0.1:15742/healthz"' not in migrated:
        raise SystemExit("update.sh did not insert panel precheck with isolated NexusHub port")
    if re.search(r"(?m)^\s*home\s*=", migrated):
        raise SystemExit("update.sh should not insert codex.home; runtime auto-discovery should apply")
    for needle in [
        "[probe]\nenabled = true\npoll_seconds = 15\nrecent_limit = 50",
        "[probe.hooks]\nmanage_stop_hook = true\nreload_app_server_after_install = true",
        "[probe.notifications]\nenabled = false\nserver_url = \"https://api.day.app\"",
        "notify_completion = true",
        "notify_reply_needed = true",
        "notify_recoverable = true",
        "[probe.observability]\nhook_event_max_lines = 500\nhook_cooldown_max_lines = 1000\nlog_max_bytes = 5242880",
        "[probe.logs_db]\nenabled = true\nretention_days = 2\nmaintenance_interval_hours = 6",
        "maintain_on_codex_exit = true",
        "codex_exit_grace_seconds = 5",
        "codex_exit_max_wait_seconds = 1800",
        "delete_chunk_rows = 5000",
        "max_delete_rows_per_run = 100000",
        "busy_timeout_ms = 500",
        "auto_compact_when_codex_closed = true",
        "compact_interval_hours = 24",
        "compact_min_freelist_mb = 256",
        "compact_min_freelist_ratio_percent = 20",
        "minimum_free_space_mb = 1024",
    ]:
        if needle not in migrated:
            raise SystemExit(f"probe default was not inserted by update migration: {needle}")

with tempfile.TemporaryDirectory() as tmp:
    config = Path(tmp) / "config.toml"
    config.write_text(textwrap.dedent("""
        [probe.observability]
        hook_event_max_lines = 120
        hook_cooldown_max_lines = 80
        log_max_bytes = 262144

        [probe.logs_db]
        enabled = true
        retention_days = 14
        maintenance_interval_hours = 24
        maintain_on_codex_exit = true
        codex_exit_grace_seconds = 10
        codex_exit_max_wait_seconds = 120
        delete_chunk_rows = 2000
        max_delete_rows_per_run = 50000
        busy_timeout_ms = 5000
        auto_compact_when_codex_closed = true
        compact_interval_hours = 168
        compact_min_freelist_mb = 64
        compact_min_freelist_ratio_percent = 20
        minimum_free_space_mb = 256
    """).lstrip())
    subprocess.run(
        ["bash", "-c", f"source {update_sh}; migrate_codex_update_config {config}"],
        check=True,
    )
    migrated_probe_defaults = config.read_text()
    for needle in [
        "hook_event_max_lines = 500",
        "hook_cooldown_max_lines = 1000",
        "log_max_bytes = 5242880",
        "retention_days = 2",
        "maintenance_interval_hours = 6",
        "codex_exit_grace_seconds = 5",
        "codex_exit_max_wait_seconds = 1800",
        "delete_chunk_rows = 5000",
        "max_delete_rows_per_run = 100000",
        "busy_timeout_ms = 500",
        "compact_interval_hours = 24",
        "compact_min_freelist_mb = 256",
        "minimum_free_space_mb = 1024",
    ]:
        if needle not in migrated_probe_defaults:
            raise SystemExit(f"old probe default was not migrated by update migration: {needle}")
    for stale in [
        "hook_event_max_lines = 120",
        "hook_cooldown_max_lines = 80",
        "log_max_bytes = 262144",
        "retention_days = 14",
        "maintenance_interval_hours = 24",
        "codex_exit_grace_seconds = 10",
        "codex_exit_max_wait_seconds = 120",
        "delete_chunk_rows = 2000",
        "max_delete_rows_per_run = 50000",
        "busy_timeout_ms = 5000",
        "compact_interval_hours = 168",
        "compact_min_freelist_mb = 64",
        "minimum_free_space_mb = 256",
    ]:
        if stale in migrated_probe_defaults:
            raise SystemExit(f"old probe default should be replaced by update migration: {stale}")

with tempfile.TemporaryDirectory() as tmp:
    config = Path(tmp) / "config.toml"
    config.write_text(textwrap.dedent('''
        [update]
        precheck_command = "codex --version && sudo -n codex --version && /usr/local/bin/codex-raw --version && readlink -f /usr/local/bin/codex && readlink -f /usr/local/bin/codex-raw && sqlite3 /root/.codex/state_5.sqlite 'pragma integrity_check;' && sqlite3 /root/.codex/state_5.sqlite \\"select count(*) total, sum(archived_at is null) active, sum(archived_at is not null) archived from threads;\\" && /home/ubuntu/codex-admin/bin/codex-cloud-doctor"
        update_command = "/usr/local/bin/nexushub-codex-update"
        prune_command = "/usr/local/bin/nexushub-codex-prune"
    ''').lstrip())
    subprocess.run(
        ["bash", "-c", f"source {update_sh}; migrate_codex_update_config {config}"],
        check=True,
    )
    migrated_precheck = config.read_text()
    if 'precheck_command = "/usr/local/bin/nexushub-codex-precheck"' not in migrated_precheck:
        raise SystemExit("update.sh did not migrate legacy codex precheck command")

with tempfile.TemporaryDirectory() as tmp:
    config = Path(tmp) / "config.toml"
    config.write_text(textwrap.dedent("""
        [paths]
        data_dir = "/srv/custom/nexushub"
        db_path = "/srv/custom/nexushub/custom.sqlite"
        webui_dir = "/srv/custom/nexushub/webui"
        log_dir = "/srv/custom/nexushub/logs"

        [update]
        update_command = "/opt/custom/update --flag"
        prune_command = "/opt/custom/prune"
    """).lstrip())
    subprocess.run(
        ["bash", "-c", f"source {update_sh}; migrate_codex_update_config {config}"],
        check=True,
    )
    preserved = config.read_text()
    if 'update_command = "/opt/custom/update --flag"' not in preserved:
        raise SystemExit("update.sh overwrote custom codex update command")
    if 'prune_command = "/opt/custom/prune"' not in preserved:
        raise SystemExit("update.sh overwrote custom codex prune command")
    if 'db_path = "/srv/custom/nexushub/custom.sqlite"' not in preserved:
        raise SystemExit("update.sh overwrote custom path config")

with tempfile.TemporaryDirectory() as tmp:
    config = Path(tmp) / "config.toml"
    config.write_text(textwrap.dedent("""
        [codex]
        home = "/srv/codex/custom-home"
        app_server_socket = "/srv/codex/custom-home/app-server-control/app-server-control.sock"
    """).lstrip())
    subprocess.run(
        ["bash", "-c", f"source {update_sh}; migrate_codex_update_config {config}"],
        check=True,
    )
    preserved_codex = config.read_text()
    if 'home = "/srv/codex/custom-home"' not in preserved_codex:
        raise SystemExit("update.sh should preserve custom codex.home")
    if 'app_server_socket = "/srv/codex/custom-home/app-server-control/app-server-control.sock"' not in preserved_codex:
        raise SystemExit("update.sh should preserve custom codex app-server socket")

with tempfile.TemporaryDirectory() as tmp:
    config = Path(tmp) / "config.toml"
    config.write_text(textwrap.dedent("""
        [probe]
        enabled = false
        poll_seconds = 45
        recent_limit = 12

        [probe.hooks]
        manage_stop_hook = false

        [probe.notifications]
        server_url = "https://bark.example"
        group = "Custom Ops"

        [probe.observability]
        log_max_bytes = 65536

        [probe.logs_db]
        retention_days = 90
        minimum_free_space_mb = 1024
    """).lstrip())
    subprocess.run(
        ["bash", "-c", f"source {update_sh}; migrate_codex_update_config {config}"],
        check=True,
    )
    preserved_probe = config.read_text()
    for needle in [
        'enabled = false',
        'poll_seconds = 45',
        'recent_limit = 12',
        'manage_stop_hook = false',
        'server_url = "https://bark.example"',
        'group = "Custom Ops"',
        'log_max_bytes = 65536',
        'retention_days = 90',
        'minimum_free_space_mb = 1024',
    ]:
        if needle not in preserved_probe:
            raise SystemExit(f"custom probe value should be preserved by update migration: {needle}")
    if 'reload_app_server_after_install = true' not in preserved_probe:
        raise SystemExit("update migration should fill missing probe.hooks defaults")
    if 'notify_recoverable = true' not in preserved_probe:
        raise SystemExit("update migration should fill missing probe.notifications defaults")
    if 'busy_timeout_ms = 500' not in preserved_probe:
        raise SystemExit("update migration should fill missing probe.logs_db defaults")

with tempfile.TemporaryDirectory() as tmp:
    config = Path(tmp) / "config.toml"
    config.write_text(textwrap.dedent("""
        [paths]
        data_dir = "/var/lib/codex-cloud-panel"
        db_path = "/var/lib/codex-cloud-panel/panel.sqlite"
        webui_dir = "/usr/share/codex-cloud-panel/webui"
        log_dir = "/var/log/codex-cloud-panel"
    """).lstrip())
    subprocess.run(
        ["bash", "-c", f"source {update_sh}; migrate_codex_update_config {config}"],
        check=True,
    )
    migrated_paths = config.read_text()
    if 'data_dir = "/opt/nexushub"' not in migrated_paths:
        raise SystemExit("update.sh did not migrate legacy paths.data_dir")
    if 'db_path = "/opt/nexushub/nexushub.sqlite"' not in migrated_paths:
        raise SystemExit("update.sh did not migrate legacy paths.db_path")
    if 'webui_dir = "/opt/nexushub/webui"' not in migrated_paths:
        raise SystemExit("update.sh did not migrate legacy paths.webui_dir")
    if 'log_dir = "/opt/nexushub/logs"' not in migrated_paths:
        raise SystemExit("update.sh did not migrate legacy paths.log_dir")

print("update.sh codex config migration behavior: ok")
PY

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT

fake_curl="${tmp}/curl"
cat > "${fake_curl}" <<'SH'
#!/usr/bin/env bash
set -Eeuo pipefail

output=""
url=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    -o)
      output="${2:-}"
      shift 2
      ;;
    *)
      url="$1"
      shift
      ;;
  esac
done

case "${url}" in
  https://api.github.com/repos/lich13/nexushub/releases/latest)
    cat > "${output}" <<'JSON'
{
  "tag_name": "v9.9.9",
  "assets": [
    {
      "name": "nexushub-linux-x86_64.tar.gz",
      "url": "https://api.github.com/assets/archive",
      "browser_download_url": "https://downloads.example/archive"
    },
    {
      "name": "nexushub-linux-x86_64.tar.gz.sha256",
      "url": "https://api.github.com/assets/sha256",
      "browser_download_url": "https://downloads.example/archive.sha256"
    }
  ]
}
JSON
    ;;
  https://api.github.com/assets/archive|https://api.github.com/assets/sha256)
    exit 22
    ;;
  https://downloads.example/archive)
    printf 'browser archive\n' > "${output}"
    ;;
  https://downloads.example/archive.sha256)
    printf 'browser sha256\n' > "${output}"
    ;;
  *)
    printf 'unexpected url: %s\n' "${url}" >&2
    exit 64
    ;;
esac
SH
chmod +x "${fake_curl}"

NEXUSHUB_CURL="${fake_curl}" bash -s "${UPDATE_SH}" "${tmp}" <<'SH'
set -Eeuo pipefail
UPDATE_SH="$1"
TMP="$2"

source "${UPDATE_SH}"
TMP="$2"
REPO="lich13/nexushub"
VERSION="latest"

download_release >/dev/null

grep -qx 'browser archive' "${ARCHIVE_PATH}"
grep -qx 'browser sha256' "${SHA256_PATH}"
SH

echo "update release download executes browser fallback after API asset failure: ok"

tmp_git="$(mktemp -d)"
trap 'rm -rf "${tmp}" "${tmp_git}"' EXIT

fake_curl_git="${tmp_git}/curl"
cat > "${fake_curl_git}" <<'SH'
#!/usr/bin/env bash
set -Eeuo pipefail

output=""
url=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    -o)
      output="${2:-}"
      shift 2
      ;;
    *)
      url="$1"
      shift
      ;;
  esac
done

case "${url}" in
  https://api.github.com/repos/lich13/nexushub/releases/latest)
    exit 22
    ;;
  https://github.com/lich13/nexushub/releases/download/v9.10.1/nexushub-linux-x86_64.tar.gz)
    printf 'git tag archive\n' > "${output}"
    ;;
  https://github.com/lich13/nexushub/releases/download/v9.10.1/nexushub-linux-x86_64.tar.gz.sha256)
    printf 'git tag sha256\n' > "${output}"
    ;;
  https://github.com/lich13/nexushub/releases/latest/download/*)
    printf 'unexpected latest fallback after git tag resolution: %s\n' "${url}" >&2
    exit 64
    ;;
  *)
    printf 'unexpected url: %s\n' "${url}" >&2
    exit 64
    ;;
esac
SH
chmod +x "${fake_curl_git}"

fake_git="${tmp_git}/git"
cat > "${fake_git}" <<'SH'
#!/usr/bin/env bash
set -Eeuo pipefail

if [[ "$1" != "ls-remote" ]]; then
  printf 'unexpected git command: %s\n' "$*" >&2
  exit 64
fi

cat <<'REFS'
1111111111111111111111111111111111111111	refs/tags/v9.9.9
2222222222222222222222222222222222222222	refs/tags/v9.10.1
3333333333333333333333333333333333333333	refs/tags/v10.0.0-beta.1
REFS
SH
chmod +x "${fake_git}"

NEXUSHUB_CURL="${fake_curl_git}" NEXUSHUB_GIT="${fake_git}" bash -s "${UPDATE_SH}" "${tmp_git}" <<'SH'
set -Eeuo pipefail
UPDATE_SH="$1"
TMP="$2"

source "${UPDATE_SH}"
TMP="$2"
REPO="lich13/nexushub"
VERSION="latest"

download_release >/dev/null

grep -qx 'git tag archive' "${ARCHIVE_PATH}"
grep -qx 'git tag sha256' "${SHA256_PATH}"
SH

echo "update release download resolves latest from git tags when GitHub API is unavailable: ok"
