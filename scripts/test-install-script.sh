#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd -P)"
cd "${ROOT}"

required=(
  deploy/nexushub-webd/install.sh
  deploy/nexushub-webd/update.sh
  deploy/nexushub-webd/rollback.sh
  deploy/nexushub-webd/web-update.sh
  scripts/package-darwin-arm64.sh
  scripts/package-webd-linux-x86_64.sh
  scripts/package-linux.sh
  scripts/deploy-cloud.sh
  contracts/nexushub-contract.json
  contracts/nexushub-contract.schema.json
  docs/ARCHITECTURE.md
  docs/cloud-deploy-runbook.md
  docs/progress/MASTER.md
)
for path in "${required[@]}"; do
  [[ -f "${ROOT}/${path}" ]] || { echo "missing required file: ${path}" >&2; exit 1; }
done
[[ ! -e "${ROOT}/scripts/package-linux-tauri-x86_64.sh" ]] || { echo "Linux Tauri package script must be retired" >&2; exit 1; }
[[ ! -e "${ROOT}/scripts/smoke-linux-tauri.py" ]] || { echo "Linux Tauri smoke must be retired" >&2; exit 1; }

python3 - "${ROOT}" <<'PY'
from pathlib import Path
import json, subprocess, sys
root = Path(sys.argv[1])
expected_docs = {
    "README.md", "AGENTS.md", "DESIGN.md",
    "docs/ARCHITECTURE.md", "docs/cloud-deploy-runbook.md", "docs/progress/MASTER.md",
}
tracked = set(subprocess.check_output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "--", "*.md"], cwd=root, text=True).splitlines())
actual = {path for path in tracked if (root / path).is_file()}
if actual != expected_docs:
    raise SystemExit(f"Markdown set mismatch: missing={sorted(expected_docs-actual)}, extra={sorted(actual-expected_docs)}")

version = json.loads((root / "package.json").read_text())["version"]
if version != "1.1.2":
    raise SystemExit(f"package version is {version}, expected 1.1.2")
for name in ["webui/package.json", "src-tauri/tauri.conf.json"]:
    if json.loads((root / name).read_text())["version"] != version:
        raise SystemExit(f"{name} version mismatch")
if 'version = "1.1.2"' not in (root / "Cargo.toml").read_text():
    raise SystemExit("Cargo workspace version mismatch")
config = json.loads((root / "src-tauri/tauri.conf.json").read_text())
if config["bundle"]["targets"] != ["dmg", "app"]:
    raise SystemExit(f"unexpected Tauri bundle targets: {config['bundle']['targets']}")

ci = (root / ".github/workflows/ci.yml").read_text()
release = (root / ".github/workflows/release.yml").read_text()
if "linux-tauri:" in ci or "linux-tauri" in release:
    raise SystemExit("Linux Tauri job remains in CI/Release")
for stale in ["AppImage", ".deb", ".rpm", '"linux-x86_64"', "xvfb-run", "package-linux-tauri-x86_64.sh"]:
    if stale in release:
        raise SystemExit(f"Release retains retired marker: {stale}")
for marker in ["webd-linux:", "macos:", "nexushub-webd-linux-x86_64.tar.gz", "darwin-aarch64", "expected seven release assets"]:
    if marker not in release:
        raise SystemExit(f"Release missing marker: {marker}")

contract = json.loads((root / "contracts/nexushub-contract.json").read_text())
active = {action.get("id") for action in contract.get("actions", [])}
if any(action.startswith("threads.goal.") for action in active):
    raise SystemExit("Goal action is still active in the contract")
retired = set(contract.get("retiredActions", []))
if not {"threads.goal.get", "threads.goal.save", "threads.goal.clear", "threads.goal.pause", "threads.goal.resume"} <= retired:
    raise SystemExit("Goal tombstones are incomplete")

print("NexusHub 1.1.2 install/release boundary checks: ok")
PY

python3 scripts/privacy-check.py --git-objects
