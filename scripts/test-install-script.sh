#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd -P)"
INSTALL_SH="${ROOT}/deploy/nexushub-webd/install.sh"
UPDATE_SH="${ROOT}/deploy/nexushub-webd/update.sh"
ROLLBACK_SH="${ROOT}/deploy/nexushub-webd/rollback.sh"
WEB_UPDATE_SH="${ROOT}/deploy/nexushub-webd/web-update.sh"
MACOS_README="${ROOT}/README.md"
PACKAGE_WEBD_SH="${ROOT}/scripts/package-webd-linux-x86_64.sh"
PACKAGE_LINUX_SH="${ROOT}/scripts/package-linux.sh"
MACOS_PACKAGE_SH="${ROOT}/scripts/package-darwin-arm64.sh"
LINUX_TAURI_PACKAGE_SH="${ROOT}/scripts/package-linux-tauri-x86_64.sh"
RELEASE_WORKFLOW="${ROOT}/.github/workflows/release.yml"
CI_WORKFLOW="${ROOT}/.github/workflows/ci.yml"
DEPLOY_CLOUD_SH="${ROOT}/scripts/deploy-cloud.sh"
CONFIG_EXAMPLE="${ROOT}/deploy/nexushub-webd/config.example.toml"
NGINX_CONF="${ROOT}/deploy/nexushub-webd/nginx.conf"
SYSTEMD_SERVICE="${ROOT}/deploy/nexushub-webd/systemd.service"
ENV_EXAMPLE="${ROOT}/deploy/nexushub-webd/env.example"
PROVIDER_PATH_RENDERER="${ROOT}/deploy/nexushub-webd/provider-session-write-paths.py"
README="${ROOT}/README.md"
CLOUD_RUNBOOK="${ROOT}/docs/cloud-deploy-runbook.md"
AGENTS="${ROOT}/AGENTS.md"
CC_SWITCH_AUDIT="${ROOT}/docs/ARCHITECTURE.md"
FEATURE_SYNC_WORKFLOW="${ROOT}/docs/ARCHITECTURE.md"
CONTRACT_REGISTRY="${ROOT}/contracts/nexushub-contract.json"
CONTRACT_SCHEMA="${ROOT}/contracts/nexushub-contract.schema.json"
CONTRACT_CHECKLIST="${ROOT}/scripts/contract-next-action-checklist.mjs"
CODEX_PRECHECK_WRAPPER="${ROOT}/deploy/nexushub-webd/nexushub-codex-precheck"
CODEX_UPDATE_WRAPPER="${ROOT}/deploy/nexushub-webd/nexushub-codex-update"
CODEX_PRUNE_WRAPPER="${ROOT}/deploy/nexushub-webd/nexushub-codex-prune"

if [[ -e "${ROOT}/desktop-ui" ]]; then
  echo "desktop-ui must stay removed; desktop Tauri uses the shared webui interface" >&2
  exit 1
fi

for path in \
  "${INSTALL_SH}" \
  "${UPDATE_SH}" \
  "${ROLLBACK_SH}" \
  "${WEB_UPDATE_SH}" \
  "${PACKAGE_WEBD_SH}" \
  "${PACKAGE_LINUX_SH}" \
  "${DEPLOY_CLOUD_SH}" \
  "${README}" \
  "${CLOUD_RUNBOOK}" \
  "${AGENTS}" \
  "${CC_SWITCH_AUDIT}" \
  "${FEATURE_SYNC_WORKFLOW}" \
  "${CONTRACT_REGISTRY}" \
  "${CONTRACT_SCHEMA}" \
  "${CONTRACT_CHECKLIST}" \
  "${CODEX_PRECHECK_WRAPPER}" \
  "${CODEX_UPDATE_WRAPPER}" \
  "${CODEX_PRUNE_WRAPPER}" \
  "${PROVIDER_PATH_RENDERER}"; do
  [[ -f "${path}" ]] || { echo "missing required file: ${path}" >&2; exit 1; }
done

python3 - "${ROOT}" <<'PY'
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote

root = Path(sys.argv[1]).resolve()
expected = {
    "AGENTS.md",
    "DESIGN.md",
    "README.md",
    "docs/ARCHITECTURE.md",
    "docs/cloud-deploy-runbook.md",
    "docs/progress/MASTER.md",
}
listed = subprocess.check_output(
    ["git", "ls-files", "--cached", "--others", "--exclude-standard", "--", "*.md"],
    cwd=root,
    text=True,
).splitlines()
present = {path for path in listed if (root / path).is_file()}
if present != expected:
    raise SystemExit(
        "repository must contain exactly six current Markdown documents; "
        f"missing={sorted(expected - present)}, extra={sorted(present - expected)}"
    )

link_pattern = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")
for relative in sorted(expected):
    source = root / relative
    for raw_target in link_pattern.findall(source.read_text(encoding="utf-8")):
        target = raw_target.strip()
        if target.startswith("<") and ">" in target:
            target = target[1:target.index(">")]
        else:
            target = target.split(maxsplit=1)[0]
        if not target or target.startswith(("#", "http://", "https://", "mailto:")):
            continue
        target = unquote(target.split("#", 1)[0].split("?", 1)[0])
        resolved = (source.parent / target).resolve()
        if not resolved.is_relative_to(root) or not resolved.exists():
            raise SystemExit(f"broken local Markdown link: {relative} -> {target}")

print("six current Markdown documents and local links: ok")
PY

bash -n "${INSTALL_SH}" "${UPDATE_SH}" "${ROLLBACK_SH}" "${WEB_UPDATE_SH}" "${PACKAGE_WEBD_SH}" "${PACKAGE_LINUX_SH}" "${DEPLOY_CLOUD_SH}"
PYTHONDONTWRITEBYTECODE=1 python3 "${ROOT}/scripts/test-tauri-build-config.py"
PYTHONDONTWRITEBYTECODE=1 python3 "${ROOT}/scripts/test-privacy-check.py"

python3 - "${CONFIG_EXAMPLE}" "${ENV_EXAMPLE}" "${SYSTEMD_SERVICE}" "${NGINX_CONF}" "${INSTALL_SH}" "${UPDATE_SH}" "${DEPLOY_CLOUD_SH}" "${PACKAGE_WEBD_SH}" "${PACKAGE_LINUX_SH}" "${README}" "${CLOUD_RUNBOOK}" "${AGENTS}" <<'PY'
from pathlib import Path
import re
import sys

config, env, systemd, nginx, install, update, deploy, package_webd, package_linux, readme, runbook, agents = [
    Path(arg).read_text() for arg in sys.argv[1:]
]

checks = {
    "config server listen": (config, 'listen = "127.0.0.1:15742"'),
    "config public URL": (config, 'public_base_url = "https://panel.example.com/nexushub/"'),
    "config data dir": (config, 'data_dir = "/var/lib/nexushub-webd"'),
    "config db": (config, 'db_path = "/var/lib/nexushub-webd/nexushub.sqlite"'),
    "config webui": (config, 'webui_dir = "/usr/share/nexushub-webd/webui"'),
    "config logs": (config, 'log_dir = "/var/log/nexushub-webd"'),
    "config panel update": (config, 'panel_update_command = "/usr/local/bin/nexushub-webd-update --repo lich13/nexushub --version latest"'),
    "config panel precheck": (config, 'panel_precheck_command = "test -x /usr/local/bin/nexushub-webd-update && systemctl is-active nexushub-webd && curl -fsS http://127.0.0.1:15742/healthz"'),
    "env config path": (env, "NEXUSHUB_CONFIG=/etc/nexushub-webd/config.toml"),
    "systemd env": (systemd, "EnvironmentFile=-/etc/nexushub-webd/env"),
    "systemd exec": (systemd, "ExecStart=/usr/local/bin/nexushub-webd --config /etc/nexushub-webd/config.toml serve --surface linux-server-webui"),
    "systemd working dir": (systemd, "WorkingDirectory=/var/lib/nexushub-webd"),
    "systemd logs": (systemd, "StandardOutput=append:/var/log/nexushub-webd/webd.log"),
    "systemd readwrite": (systemd, "ReadWritePaths=/root/.codex /home/ubuntu/.codex -/root/.grok/sessions -/root/.pi/agent/sessions /etc/nexushub-webd /var/lib/nexushub-webd /var/log/nexushub-webd"),
    "nginx scoped api": (nginx, "location ^~ /nexushub/api/"),
    "nginx scoped probe 404": (nginx, "location ^~ /nexushub/api/probe/"),
    "nginx metrics 404": (nginx, "location ^~ /nexushub/metrics"),
    "nginx no root api": (nginx, "location = /api/sentinel/status"),
    "nginx proxy": (nginx, "proxy_pass http://127.0.0.1:15742/;"),
    "install copy legacy": (install, "copy_legacy_runtime_once"),
    "install legacy dir": (install, 'LEGACY_DIR="/opt/nexushub"'),
    "install new service": (install, 'SERVICE_NAME="nexushub-webd"'),
    "install update bin": (install, 'UPDATE_BIN="/usr/local/bin/${APP_NAME}-update"'),
    "install old service disable": (install, 'systemctl disable "${LEGACY_SERVICE}.service"'),
    "install provider path renderer": (install, 'provider-session-write-paths.py" "${ENV_FILE}"'),
    "install provider path drop-in": (install, '20-provider-session-paths.conf'),
    "install provider drop-in symlink rejection": (install, '|| -L "${PROVIDER_PATHS_DROPIN}"'),
    "update asset": (update, 'ASSET="nexushub-webd-linux-${ARCH}.tar.gz"'),
    "update repo option": (update, '--repo'),
    "update version option": (update, '--version'),
    "update precheck option": (update, '--precheck'),
    "deploy default archive": (deploy, 'ARCHIVE="${2:-dist/nexushub-webd-linux-x86_64.tar.gz}"'),
    "deploy isolated staging": (deploy, 'mktemp -d /tmp/nexushub-deploy.XXXXXXXX'),
    "deploy explicit domain": (deploy, 'NEXUSHUB_DOMAIN:?'),
    "deploy explicit host": (deploy, 'HOST="${1:?'),
    "deploy new service": (deploy, "systemctl is-active --quiet nexushub-webd"),
    "deploy new binary": (deploy, "/usr/local/bin/nexushub-webd --version"),
    "package asset": (package_webd, 'ASSET="nexushub-webd-linux-x86_64.tar.gz"'),
    "package root": (package_webd, 'ARCHIVE_ROOT="nexushub-webd-linux-x86_64"'),
    "package layout check": (package_webd, '"${TMP}/${ARCHIVE_ROOT}/deploy/install.sh" --check'),
    "old package shim": (package_linux, "package-webd-linux-x86_64.sh"),
}

missing = [name for name, (text, needle) in checks.items() if needle not in text]
if missing:
    raise SystemExit("webd deploy guard missing: " + ", ".join(missing))

for name, text in {
    "config.example.toml": config,
    "systemd.service": systemd,
    "nginx.conf": nginx,
    "scripts/deploy-cloud.sh": deploy,
    "scripts/package-webd-linux-x86_64.sh": package_webd,
}.items():
    if "127.0.0.1:15732" in text:
        raise SystemExit(f"{name} must not use legacy codex-cloud-panel port 15732")
    if "nexushub-linux-x86_64.tar.gz" in text:
        raise SystemExit(f"{name} must not reference removed server asset nexushub-linux-x86_64.tar.gz")

for name, text in {
    "systemd.service": systemd,
    "config.example.toml": config,
}.items():
    for forbidden in ["codex-app-server-root.service", "app_server_socket", "bridge_enabled", "bridge_transport"]:
        if forbidden in text:
            raise SystemExit(f"{name} must not require retired app-server bridge key: {forbidden}")

if "location ^~ /api/" in nginx:
    raise SystemExit("nginx.conf must not proxy root /api/ because Sub2API owns that namespace")
if "/v1" in nginx and "return 404" not in nginx:
    raise SystemExit("nginx.conf must keep sensitive /v1 paths unavailable")
if 'trap \'rm -rf "${tmp}"\' RETURN' in install and "trap - RETURN" not in install:
    raise SystemExit("install.sh RETURN trap must be cleared before local tmp leaves scope")

for doc_name, doc in {"README.md": readme, "docs/cloud-deploy-runbook.md": runbook}.items():
    for stale in [
        "nexushub-linux-x86_64.tar.gz",
        "sudo systemctl is-active nexushub\n",
        "/opt/nexushub/bin/nexushub-webd",
        "/usr/local/bin/nexushub-update --repo lich13/nexushub --version latest",
    ]:
        if stale in doc:
            raise SystemExit(f"{doc_name} must not document stale server surface: {stale}")
    for needle in [
        "nexushub-webd-linux-x86_64.tar.gz",
        "/usr/local/bin/nexushub-webd",
        "/etc/nexushub-webd/config.toml",
        "/var/lib/nexushub-webd",
        "nexushub-webd.service",
    ]:
        if needle not in doc:
            raise SystemExit(f"{doc_name} missing current webd surface: {needle}")
if "contract registry" not in agents or "contracts/nexushub-contract.json" not in agents:
    raise SystemExit("AGENTS.md must document the shared contract registry workflow")
if "contract-next-action-checklist" not in agents:
    raise SystemExit("AGENTS.md must require the contract next-action checklist before new feature work")

print("webd deploy layout: ok")
PY

PYTHONDONTWRITEBYTECODE=1 python3 - "${PROVIDER_PATH_RENDERER}" <<'PY'
from pathlib import Path
import subprocess
import sys
import tempfile

renderer = Path(sys.argv[1])

def render(environment: str, *, expect_ok: bool = True, symlink: bool = False) -> tuple[str, str]:
    with tempfile.TemporaryDirectory(prefix="nexushub-provider-paths-") as temporary:
        root = Path(temporary)
        if symlink:
            (root / "actual").mkdir()
            (root / "link").symlink_to(root / "actual", target_is_directory=True)
            environment = environment.replace("__SYMLINK__", str(root / "link"))
        env_file = root / "env"
        output = root / "drop-in.conf"
        env_file.write_text(environment, encoding="utf-8")
        result = subprocess.run(
            [sys.executable, str(renderer), str(env_file), str(output)],
            text=True,
            capture_output=True,
            check=False,
        )
        if (result.returncode == 0) != expect_ok:
            raise SystemExit(f"provider path renderer unexpected result: {result.returncode}: {result.stderr}")
        return (output.read_text(encoding="utf-8") if output.exists() else "", result.stderr)

default_output, _ = render("GROK_HOME=/root/.grok\nPI_CODING_AGENT_DIR=/root/.pi/agent\n")
if default_output:
    raise SystemExit("default provider roots must rely on the main unit and produce no drop-in")

custom_output, _ = render(
    'GROK_HOME="/srv/grok home"\n'
    'PI_CODING_AGENT_DIR=/srv/pi-agent\n'
    'PI_CODING_AGENT_SESSION_DIR=/srv/pi-sessions\n'
)
expected = (
    '[Service]\n'
    'ReadWritePaths="-/srv/grok home/sessions"\n'
    'ReadWritePaths="-/srv/pi-sessions"\n'
)
if custom_output != expected:
    raise SystemExit(f"custom provider drop-in mismatch: {custom_output!r}")
if "/srv/pi-agent/sessions" in custom_output:
    raise SystemExit("PI_CODING_AGENT_SESSION_DIR must take precedence over PI_CODING_AGENT_DIR")

for invalid in ["GROK_HOME=relative\n", "PI_CODING_AGENT_SESSION_DIR=/root\n"]:
    _, error = render(invalid, expect_ok=False)
    if "provider session write path error" not in error:
        raise SystemExit("invalid provider root must return an actionable renderer error")

_, error = render("PI_CODING_AGENT_SESSION_DIR=__SYMLINK__/sessions\n", expect_ok=False, symlink=True)
if "traverses a symlink" not in error:
    raise SystemExit("provider path renderer must reject symlink traversal")

print("provider session ReadWritePaths renderer: ok")
PY

python3 - "${CONTRACT_REGISTRY}" "${CONTRACT_SCHEMA}" "${CC_SWITCH_AUDIT}" "${FEATURE_SYNC_WORKFLOW}" "${README}" "${CLOUD_RUNBOOK}" "${AGENTS}" <<'PY'
from pathlib import Path
import json
import sys

registry_path, schema_path, audit_path, workflow_path, readme_path, runbook_path, agents_path = [Path(arg) for arg in sys.argv[1:]]
registry = json.loads(registry_path.read_text())
schema = json.loads(schema_path.read_text())
audit = audit_path.read_text()
workflow = workflow_path.read_text()
readme = readme_path.read_text()
runbook = runbook_path.read_text()
agents = agents_path.read_text()

required_top_level = [
    "schemaVersion",
    "hostSurfaces",
    "capabilities",
    "capabilitiesByHostSurface",
    "visual",
    "actions",
    "retiredActions",
    "dtoCatalog",
]
if schema.get("$id") != "https://github.com/lich13/nexushub/contracts/nexushub-contract.schema.json":
    raise SystemExit("contract schema must have the canonical GitHub $id")
if schema.get("required") != required_top_level:
    raise SystemExit("contract schema required keys must match the registry top-level shape")
if list(schema.get("properties", {}).keys()) != [
    "retiredActions",
    "schemaVersion",
    "hostSurfaces",
    "capabilities",
    "capabilitiesByHostSurface",
    "visual",
    "actions",
    "dtoCatalog",
]:
    raise SystemExit("contract schema properties must stay in the canonical order")

host_surfaces = registry.get("hostSurfaces", [])
if host_surfaces != ["linux_server_webui", "desktop_embedded_tauri"]:
    raise SystemExit("contract registry host surfaces drifted from the accepted two-surface model")
for surface in host_surfaces:
    if surface not in registry.get("capabilitiesByHostSurface", {}):
        raise SystemExit(f"contract registry missing capability matrix for {surface}")
dto_catalog = registry.get("dtoCatalog", {})
if not isinstance(dto_catalog, dict) or not dto_catalog:
    raise SystemExit("contract registry must declare a non-empty dtoCatalog")
for dto_name, dto_entry in dto_catalog.items():
    if not isinstance(dto_entry, dict) or not dto_entry.get("core") or not dto_entry.get("webui"):
        raise SystemExit(f"contract dtoCatalog entry {dto_name} must declare core and webui names")
for action in registry.get("actions", []):
    action_id = action.get("id")
    if action.get("scope") == "shared":
        for key in ["coreUseCase", "linuxRpc", "tauriCommand", "webuiWrapper", "dtoOwner", "requestDto", "responseDto"]:
            if not action.get(key):
                raise SystemExit(f"shared contract action {action_id} missing {key}")
    if action.get("scope") == "transport":
        for key in ["webuiWrapper", "dtoOwner", "requestDto", "responseDto"]:
            if not action.get(key):
                raise SystemExit(f"transport contract action {action_id} missing {key}")
    if action.get("scope") in {"shared", "transport"}:
        for key in ["requestDto", "responseDto"]:
            dto_name = action.get(key)
            if dto_name not in dto_catalog:
                raise SystemExit(f"contract action {action_id} references unknown {key} {dto_name}")
    if action.get("scope") == "host_only" and not action.get("hostOnlyReason"):
        raise SystemExit(f"host-only contract action {action_id} missing hostOnlyReason")

for needle in [
    "cc-switch origin/main",
    "cc-switch feat/webd",
    "Windows desktop",
    "Linux arm64",
    "nexushub-webd-linux-x86_64.tar.gz",
    "NexusHub-*-Linux-x86_64.AppImage",
    "contracts/nexushub-contract.json",
]:
    if needle not in audit:
        raise SystemExit(f"cc-switch architecture audit missing {needle}")

for needle in [
    "contract-next-action-checklist.mjs",
    "contracts/nexushub-contract.json",
    "NexusHubUseCases",
    "contract_dtos.rs",
    "contractDtoMap.ts",
    "Linux RPC",
    "Tauri command",
    "Browser",
    "Computer Use",
]:
    if needle not in workflow:
        raise SystemExit(f"feature sync workflow missing {needle}")

for doc_name, doc in {
    "README.md": readme,
    "docs/cloud-deploy-runbook.md": runbook,
}.items():
    for needle in [
        "nexushub-webd-linux-x86_64.tar.gz",
        "NexusHub-<version>-Linux-x86_64.AppImage",
        "latest.json",
        "WebKit/GTK",
        "xvfb",
    ]:
        if needle not in doc:
            raise SystemExit(f"{doc_name} missing Linux asset/build-time boundary: {needle}")
    if "headless webd tarball into `latest.json`" not in doc and "headless webd tarball is not a Tauri updater asset" not in doc:
        raise SystemExit(f"{doc_name} must say the headless webd tarball is not a latest.json asset")
for needle in [
    "contract registry",
    "contracts/nexushub-contract.json",
    "contracts/nexushub-contract.schema.json",
    "contract-next-action-checklist",
    "dtoOwner",
    "Windows",
    "Linux arm64",
]:
    if needle not in agents:
        raise SystemExit(f"AGENTS.md missing governance marker: {needle}")

print("contract schema and cc-switch architecture audit guards: ok")
PY

checklist_output="$(node "${CONTRACT_CHECKLIST}" grok.rename)"
for needle in \
  "NexusHub contract-driven next action checklist" \
  "scope: shared" \
  "requestDto=GrokRenameRequest" \
  "responseDto=GrokMutationResponse" \
  "core use-case/DTO" \
  "Linux RPC" \
  "Tauri command" \
  "WebUI wrapper" \
  "Browser for Linux WebUI" \
  "Computer Use for macOS Tauri"; do
  [[ "${checklist_output}" == *"${needle}"* ]] || { echo "contract checklist output missing: ${needle}" >&2; exit 1; }
done
pi_checklist_output="$(node "${CONTRACT_CHECKLIST}" pi.rename)"
for needle in \
  "scope: shared" \
  "requestDto=PiRenameRequest" \
  "responseDto=PiMutationResponse" \
  "core use-case/DTO" \
  "Linux RPC" \
  "Tauri command" \
  "WebUI wrapper"; do
  [[ "${pi_checklist_output}" == *"${needle}"* ]] || { echo "Pi contract checklist output missing: ${needle}" >&2; exit 1; }
done

python3 - "${INSTALL_SH}" <<'PY'
from pathlib import Path
import re
import subprocess
import tempfile
import textwrap
import sys

install = Path(sys.argv[1]).read_text()
match = re.search(r"python3 - \"\$\{CONFIG_FILE\}\" <<'PY'\n(?P<body>.*?)\nPY", install, re.S)
if not match:
    raise SystemExit("install.sh config migration Python block not found")
block = match.group("body")

def migrate(text: str) -> str:
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "config.toml"
        path.write_text(text)
        subprocess.run(["python3", "-", str(path)], input=block, text=True, check=True)
        return path.read_text()

legacy = textwrap.dedent("""
    [server]
    listen = "127.0.0.1:15732"

    [paths]
    data_dir = "/opt/nexushub"
    db_path = "/opt/nexushub/nexushub.sqlite"
    webui_dir = "/opt/nexushub/webui"
    log_dir = "/opt/nexushub/logs"

    [update]
    panel_update_command = "/usr/local/bin/nexushub-update --repo lich13/nexushub --version latest"
    panel_precheck_command = "test -x /usr/local/bin/nexushub-update && systemctl is-active nexushub && curl -fsS http://127.0.0.1:15742/healthz"
""").lstrip()
migrated = migrate(legacy)
for needle in [
    'listen = "127.0.0.1:15742"',
    'data_dir = "/var/lib/nexushub-webd"',
    'db_path = "/var/lib/nexushub-webd/nexushub.sqlite"',
    'webui_dir = "/usr/share/nexushub-webd/webui"',
    'log_dir = "/var/log/nexushub-webd"',
    'panel_update_command = "/usr/local/bin/nexushub-webd-update --repo lich13/nexushub --version latest"',
    'panel_precheck_command = "test -x /usr/local/bin/nexushub-webd-update && systemctl is-active nexushub-webd && curl -fsS http://127.0.0.1:15742/healthz"',
]:
    if needle not in migrated:
        raise SystemExit(f"install migration missing {needle}")
for stale in [
    'data_dir = "/opt/nexushub"',
    'db_path = "/opt/nexushub/nexushub.sqlite"',
    'panel_update_command = "/usr/local/bin/nexushub-update --repo lich13/nexushub --version latest"',
]:
    if stale in migrated:
        raise SystemExit(f"install migration kept stale value: {stale}")

custom = textwrap.dedent("""
    [paths]
    data_dir = "/srv/custom/nexushub"
    db_path = "/srv/custom/nexushub/custom.sqlite"
    webui_dir = "/srv/custom/nexushub/webui"
    log_dir = "/srv/custom/nexushub/logs"
""").lstrip()
custom_migrated = migrate(custom)
if 'db_path = "/srv/custom/nexushub/custom.sqlite"' not in custom_migrated:
    raise SystemExit("install migration should preserve custom paths")

print("install config migration to webd layout: ok")
PY

python3 - "${CODEX_PRECHECK_WRAPPER}" "${CODEX_UPDATE_WRAPPER}" "${CODEX_PRUNE_WRAPPER}" <<'PY'
from pathlib import Path
import sys

wrappers = [Path(arg) for arg in sys.argv[1:]]
for path in wrappers:
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
            raise SystemExit(f"{path.name} missing controlled systemd-run marker: {needle}")
for path, commands in {
    wrappers[0]: ["sudo -n codex --version", "sudo -n codex mcp list", "/usr/local/bin/codex-raw --version", "/home/ubuntu/codex-admin/bin/codex-cloud-doctor"],
    wrappers[1]: ["/home/ubuntu/codex-admin/bin/codex-cloud-update --no-prune", "/home/ubuntu/codex-admin/bin/codex-cloud-prune", "codex --version", "codex mcp list"],
    wrappers[2]: ["/home/ubuntu/codex-admin/bin/codex-cloud-prune"],
}.items():
    text = path.read_text()
    for command in commands:
        if command not in text:
            raise SystemExit(f"{path.name} missing command: {command}")
print("codex transient wrappers: ok")
PY

python3 - "${DEPLOY_CLOUD_SH}" <<'PY'
from pathlib import Path
import os
import subprocess
import sys
import tempfile

deploy = Path(sys.argv[1])
root = deploy.parents[1]

with tempfile.TemporaryDirectory() as tmp:
    tmp_path = Path(tmp)
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    log = tmp_path / "commands.log"
    archive = tmp_path / "nexushub-webd-linux-x86_64.tar.gz"
    archive.write_bytes(b"fake archive")

    fake_commands = {
        "scp": "printf 'scp %s\\n' \"$*\" >> \"$NEXUSHUB_FAKE_DEPLOY_LOG\"\n",
        "ssh": "printf 'ssh %s\\n' \"$*\" >> \"$NEXUSHUB_FAKE_DEPLOY_LOG\"\nif [[ \"$*\" == *'mktemp -d'* ]]; then printf '/tmp/nexushub-deploy.fixture\\n'; fi\n",
        "tar": "printf 'tar %s\\n' \"$*\" >> \"$NEXUSHUB_FAKE_DEPLOY_LOG\"\n",
        "tr": "cat\n",
        "grep": "exit 1\n",
        "curl": """
printf 'curl %s\\n' "$*" >> "$NEXUSHUB_FAKE_DEPLOY_LOG"
for arg in "$@"; do
  case "$arg" in
    *codex-cloud-panel*|*api/sentinel/status*|*api/probe/status*|*api/v1/models*)
      printf '404'
      exit 0
      ;;
  esac
done
printf '200'
exit 0
""",
    }
    for name, body in fake_commands.items():
        path = fake_bin / name
        path.write_text(f"#!/usr/bin/env bash\nset -Eeuo pipefail\n{body}")
        path.chmod(0o755)

    env = os.environ.copy()
    env["PATH"] = f"{fake_bin}:{env['PATH']}"
    env["NEXUSHUB_DOMAIN"] = "panel.example.com"
    env["NEXUSHUB_FAKE_DEPLOY_LOG"] = str(log)
    env["NEXUSHUB_FAKE_DEPLOY_TMP"] = str(tmp_path)
    subprocess.run(
        ["bash", str(deploy), "fake-host", str(archive)],
        cwd=root,
        env=env,
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )
    recorded = log.read_text()

for needle in [
    "scp " + str(archive),
    "/tmp/nexushub-deploy.fixture/release.tar.gz",
    "nexushub-webd/install.sh",
    "systemctl is-active --quiet nexushub-webd",
    "/usr/local/bin/nexushub-webd --version",
    "https://panel.example.com/nexushub/",
    "https://panel.example.com/codex-cloud-panel/",
    "https://panel.example.com/api/sentinel/status",
    "https://panel.example.com/api/probe/status",
    "https://panel.example.com/nexushub/api/sentinel/status",
]:
    if needle not in recorded:
        raise SystemExit(f"deploy-cloud.sh smoke simulation missed {needle}")
for stale in ["/opt/nexushub/bin/nexushub-webd", "systemctl is-active --quiet nexushub "]:
    if stale in recorded:
        raise SystemExit(f"deploy-cloud.sh smoke simulation kept stale command: {stale}")
print("deploy-cloud.sh webd smoke checks: ok")
PY

python3 - "${RELEASE_WORKFLOW}" "${CI_WORKFLOW}" "${PACKAGE_WEBD_SH}" <<'PY'
from pathlib import Path
import re
import sys

release, ci, package_webd = [Path(arg).read_text() for arg in sys.argv[1:]]

for needle in [
    "cargo fmt --all -- --check",
    "cargo test --workspace",
    "cargo clippy --workspace --all-targets -- -D warnings",
    "corepack pnpm@11.0.8 --dir webui typecheck",
    "corepack pnpm@11.0.8 --dir webui test",
    "corepack pnpm@11.0.8 --dir webui build",
    "bash scripts/test-install-script.sh",
]:
    if needle not in release:
        raise SystemExit(f"release guard missing {needle}")
if "scripts/package-webd-linux-x86_64.sh" not in release:
    raise SystemExit("release workflow must build the webd Linux tarball with scripts/package-webd-linux-x86_64.sh")
for needle in [
    "concurrency:",
    "linux-x86_64:",
    "name: Linux x86_64 desktop and webd assets",
    "nexushub-linux-x86_64-assets",
    "SKIP_WEBUI_INSTALL=1 bash scripts/package-webd-linux-x86_64.sh",
    "actions/cache@v4",
    "corepack pnpm@11.0.8 store path --silent",
    "cargo-release-linux-x86_64",
]:
    if needle not in release:
        raise SystemExit(f"release workflow missing cc-switch style Linux/cache marker: {needle}")
for stale in [
    "\n  linux:\n",
    "\n  linux-tauri-x86_64:\n",
    "name: nexushub-webd-linux-x86_64",
    "name: nexushub-linux-tauri-x86_64",
]:
    if stale in release:
        raise SystemExit(f"release workflow must not keep split Linux release job/artifact marker: {stale.strip()}")
for needle in [
    "dist/nexushub-webd-linux-x86_64.tar.gz",
    "dist/nexushub-webd-linux-x86_64.tar.gz.sha256",
    "dist/NexusHub-*-Linux-x86_64.AppImage",
    "dist/NexusHub-*-Linux-x86_64.AppImage.sig",
    "dist/NexusHub-*-Linux-x86_64.AppImage.sha256",
    "dist/NexusHub-*-Linux-x86_64.deb",
    "dist/NexusHub-*-Linux-x86_64.deb.sha256",
    "dist/NexusHub-*-Linux-x86_64.rpm",
    "dist/NexusHub-*-Linux-x86_64.rpm.sha256",
    "dist/latest.json",
]:
    if needle not in release:
        raise SystemExit(f"release workflow missing asset: {needle}")
linux_job = re.search(r"\n  linux-x86_64:\n(?P<body>.*?)\n  macos-darwin-arm64:", release, re.S)
if not linux_job:
    raise SystemExit("release workflow must keep a single linux-x86_64 job for desktop and webd assets")
linux_job_body = linux_job.group("body")
for needle in [
    "bash scripts/package-linux-tauri-x86_64.sh",
    "SKIP_WEBUI_INSTALL=1 bash scripts/package-webd-linux-x86_64.sh",
    "Smoke Linux Tauri AppImage with xvfb",
]:
    if needle not in linux_job_body:
        raise SystemExit(f"linux-x86_64 release job missing {needle}")
latest_json_match = re.search(
    r"- name: Generate updater latest\.json(?P<body>.*?)- uses: softprops/action-gh-release@v2",
    release,
    re.S,
)
if not latest_json_match:
    raise SystemExit("release workflow missing Generate updater latest.json step")
latest_json_block = latest_json_match.group("body")
for needle in [
    '"darwin-aarch64"',
    '"linux-x86_64"',
    "linux_appimage",
    "NexusHub-${version}-Linux-x86_64.AppImage",
]:
    if needle not in latest_json_block:
        raise SystemExit(f"latest.json generation missing Tauri updater marker: {needle}")
if "nexushub-webd-linux-x86_64.tar.gz" in latest_json_block:
    raise SystemExit("latest.json must not use the headless webd tarball as a Tauri updater asset")
if "dist/nexushub-linux-x86_64.tar.gz" in release:
    raise SystemExit("release workflow must not upload removed nexushub-linux_x86_64 server tarball")
if "nexushub-linux_x86_64" in release:
    raise SystemExit("release workflow must not use old server artifact name")
for needle in [
    "concurrency:",
    "frontend:",
    "backend:",
    "runs-on: ubuntu-24.04",
    "linux-tauri",
    "actions/cache@v4",
    "corepack pnpm@11.0.8 store path --silent",
    "xvfb-run",
    'TAURI_CONFIG=\'{"bundle":{"resources":["resources/nexushub-webd"]}}\'',
]:
    if needle not in ci:
        raise SystemExit(f"CI workflow missing {needle}")
if "nexushub-linux-x86_64.tar.gz" in package_webd:
    raise SystemExit("webd package script must not mention old tarball name")
print("CI/Release webd asset guards: ok")
PY

python3 - "${ROOT}/src-tauri/resources/nexushub-webd" "${ROOT}/src-tauri/tauri.conf.json" "${MACOS_PACKAGE_SH}" "${LINUX_TAURI_PACKAGE_SH}" "${MACOS_README}" <<'PY'
from pathlib import Path
import subprocess
import sys

helper, tauri_config, mac_package, linux_package, macos_readme = [Path(arg) for arg in sys.argv[1:]]
if not helper.exists():
    raise SystemExit("src-tauri/resources/nexushub-webd placeholder missing")
if helper.stat().st_size > 4096:
    raise SystemExit("src-tauri/resources/nexushub-webd placeholder must stay small")
if not helper.read_text(errors="ignore").startswith("NEXUSHUB_HELPER_PLACEHOLDER"):
    raise SystemExit("src-tauri/resources/nexushub-webd must stay placeholder text in git")
kind = subprocess.check_output(["file", str(helper)], text=True)
if "Mach-O" in kind or "ELF" in kind:
    raise SystemExit("src-tauri/resources/nexushub-webd must not be a binary in git")
config = tauri_config.read_text()
for needle in ['"resources/nexushub-webd": "nexushub-webd"', '"frontendDist": "../webui/dist"']:
    if needle not in config:
        raise SystemExit(f"tauri.conf.json missing {needle}")
if '"../webui/dist": "webui"' in config:
    raise SystemExit("tauri.conf.json must not bundle a retired desktop webui asset directory")
for script in [mac_package, linux_package]:
    text = script.read_text()
    for needle in ["cargo build --release --package nexushub-webd", 'HELPER_RESOURCE="${TAURI_DIR}/resources/nexushub-webd"', "restore_helper_resource"]:
        if needle not in text:
            raise SystemExit(f"{script.name} missing {needle}")
for forbidden in ["LaunchAgent", "Cloudflare Tunnel", "com.nexushub.nexushub.plist"]:
    if forbidden in macos_readme.read_text():
        raise SystemExit(f"macOS README must not restore retired desktop Web service: {forbidden}")
print("Tauri helper resource guards: ok")
PY

echo "NexusHub install/static guards: ok"

python3 "${ROOT}/scripts/privacy-check.py"
