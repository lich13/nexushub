#!/usr/bin/env python3
"""Exercise installer archive and rollback behavior with isolated fixtures."""

from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shlex
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
SAFETY_PATH = ROOT / "deploy/nexushub-webd/install-safety.py"
INSTALL_PATH = ROOT / "deploy/nexushub-webd/install.sh"
UPDATE_PATH = ROOT / "deploy/nexushub-webd/update.sh"
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("install_safety", SAFETY_PATH)
if spec is None or spec.loader is None:
    raise RuntimeError("could not load installer safety module")
safety = importlib.util.module_from_spec(spec)
spec.loader.exec_module(safety)


def add_member(archive: tarfile.TarFile, name: str, kind: bytes, data: bytes = b"", mode: int = 0o644, linkname: str = "") -> None:
    member = tarfile.TarInfo(name)
    member.type = kind
    member.mode = mode
    member.linkname = linkname
    if kind in (tarfile.REGTYPE, tarfile.AREGTYPE):
        member.size = len(data)
        archive.addfile(member, io.BytesIO(data))
    else:
        archive.addfile(member)


def make_archive(path: Path, members: list[tuple[str, bytes, bytes, int, str]]) -> None:
    with tarfile.open(path, "w:gz") as archive:
        for name, kind, data, mode, linkname in members:
            add_member(archive, name, kind, data, mode, linkname)


class InstallSafetyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory(prefix="nexushub-install-test-")
        self.root = Path(self.temp.name)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def assert_archive_rejected(self, path: Path) -> None:
        destination = self.root / f"extract-{path.name}"
        destination.mkdir(parents=True)
        with self.assertRaises((ValueError, OSError, tarfile.TarError, EOFError)):
            safety.extract(path, destination)
        self.assertEqual(list(destination.iterdir()), [])

    def test_checksum_rejects_bad_digest_wrong_target_and_multiple_lines(self) -> None:
        archive = self.root / "release.tar.gz"
        archive.write_bytes(b"synthetic archive bytes")
        good = hashlib.sha256(archive.read_bytes()).hexdigest()
        checksum = self.root / "release.sha256"

        checksum.write_text(f"{'0' * 64}  {archive.name}\n", encoding="ascii")
        with self.assertRaises(ValueError):
            safety.verify(archive, checksum)

        checksum.write_text(f"{good}  other.tar.gz\n", encoding="ascii")
        with self.assertRaises(ValueError):
            safety.verify(archive, checksum)

        checksum.write_text(f"{good}  {archive.name}\n{good}  {archive.name}\n", encoding="ascii")
        with self.assertRaises(ValueError):
            safety.verify(archive, checksum)

    def test_extract_rejects_corrupt_gzip(self) -> None:
        archive = self.root / "broken.tar.gz"
        archive.write_bytes(b"not a gzip archive")
        self.assert_archive_rejected(archive)

    def test_extract_rejects_unsafe_paths_and_archive_member_types(self) -> None:
        bad_members = {
            "parent traversal": [("../escape", tarfile.REGTYPE, b"x", 0o644, "")],
            "absolute path": [("/escape", tarfile.REGTYPE, b"x", 0o644, "")],
            "symlink": [("payload/link", tarfile.SYMTYPE, b"", 0o777, "target")],
            "hardlink": [("payload/link", tarfile.LNKTYPE, b"", 0o644, "payload/file")],
            "device": [("payload/device", tarfile.CHRTYPE, b"", 0o600, "")],
            "duplicate name": [
                ("payload/file", tarfile.REGTYPE, b"first", 0o644, ""),
                ("payload/file", tarfile.REGTYPE, b"second", 0o644, ""),
            ],
            "file is parent": [
                ("payload", tarfile.REGTYPE, b"file", 0o644, ""),
                ("payload/child", tarfile.REGTYPE, b"child", 0o644, ""),
            ],
        }
        for label, members in bad_members.items():
            with self.subTest(label=label):
                archive = self.root / f"{label.replace(' ', '-')}.tar.gz"
                make_archive(archive, members)
                self.assert_archive_rejected(archive)

    def test_extract_accepts_regular_payload_and_preserves_executable_mode(self) -> None:
        archive = self.root / "safe.tar.gz"
        payload = b"synthetic executable payload"
        make_archive(
            archive,
            [
                ("nexushub-webd-linux-x86_64", tarfile.DIRTYPE, b"", 0o755, ""),
                ("nexushub-webd-linux-x86_64/bin/nexushub-webd", tarfile.REGTYPE, payload, 0o751, ""),
                ("nexushub-webd-linux-x86_64/deploy/update.sh", tarfile.REGTYPE, b"#!/bin/sh\n", 0o755, ""),
            ],
        )
        destination = self.root / "extract"
        safety.extract(archive, destination)
        binary = destination / "nexushub-webd-linux-x86_64/bin/nexushub-webd"
        self.assertEqual(binary.read_bytes(), payload)
        self.assertEqual(stat.S_IMODE(binary.stat().st_mode), 0o751)

    def test_atomic_copy_replaces_file_and_applies_requested_mode(self) -> None:
        source = self.root / "source"
        target = self.root / "target"
        source.write_bytes(b"new content")
        target.write_bytes(b"old content")
        target.chmod(0o600)

        safety.atomic_copy(source, target, "0751")

        self.assertEqual(target.read_bytes(), b"new content")
        self.assertEqual(stat.S_IMODE(target.stat().st_mode), 0o751)

    def test_atomic_copy_failure_keeps_old_file_and_cleans_temporary_file(self) -> None:
        source = self.root / "source"
        target = self.root / "target"
        source.write_bytes(b"new content")
        target.write_bytes(b"old content")
        before = set(self.root.iterdir())

        with mock.patch.object(safety.os, "replace", side_effect=OSError("synthetic replace failure")):
            with self.assertRaises(OSError):
                safety.atomic_copy(source, target, "0640")

        self.assertEqual(target.read_bytes(), b"old content")
        self.assertEqual(set(self.root.iterdir()), before)

    def test_rollback_restores_old_files_removes_new_files_and_leaves_database_alone(self) -> None:
        transaction = self.root / "transaction"
        transaction.mkdir()
        managed = self.root / "managed"
        managed.mkdir()
        old_file = managed / "existing.conf"
        new_file = managed / "new-wrapper"
        database = self.root / "data/business.sqlite"
        database.parent.mkdir()
        old_file.write_text("before\n", encoding="utf-8")
        old_file.chmod(0o640)
        database.write_bytes(b"independent fixture database bytes")
        database.chmod(0o600)

        safety.snapshot(transaction, old_file)
        safety.snapshot(transaction, new_file)
        source = self.root / "replacement"
        source.write_text("after\n", encoding="utf-8")
        safety.atomic_copy(source, old_file, "0600")
        safety.atomic_copy(source, new_file, "0755")
        database_before = (database.read_bytes(), stat.S_IMODE(database.stat().st_mode))

        safety.rollback(transaction)

        self.assertEqual(old_file.read_text(encoding="utf-8"), "before\n")
        self.assertEqual(stat.S_IMODE(old_file.stat().st_mode), 0o640)
        self.assertFalse(new_file.exists())
        self.assertEqual((database.read_bytes(), stat.S_IMODE(database.stat().st_mode)), database_before)
        manifest = json.loads((transaction / "files.json").read_text(encoding="utf-8"))
        self.assertNotIn(str(database), [entry["target"] for entry in manifest])

    def test_install_main_rolls_back_after_service_start_failure_without_touching_system(self) -> None:
        managed = self.root / "managed"
        bin_dir = managed / "bin"
        share_dir = managed / "share"
        config_dir = managed / "config"
        data_dir = managed / "data"
        log_dir = managed / "log"
        systemd_dir = managed / "systemd"
        dropin_dir = systemd_dir / "nexushub-webd.service.d"
        legacy_dir = managed / "legacy"
        for directory in (bin_dir, share_dir, config_dir, data_dir, log_dir, dropin_dir, legacy_dir):
            directory.mkdir(parents=True, exist_ok=True)

        install_bin = bin_dir / "nexushub-webd"
        update_bin = bin_dir / "nexushub-webd-update"
        config = config_dir / "config.toml"
        env_file = config_dir / "env"
        unit = systemd_dir / "nexushub-webd.service"
        dropin = dropin_dir / "20-provider-session-paths.conf"
        safety_copy = share_dir / "install-safety.py"
        wrappers = [
            bin_dir / "nexushub-codex-precheck",
            bin_dir / "nexushub-codex-update",
            bin_dir / "nexushub-codex-prune",
        ]
        database = data_dir / "nexushub.sqlite"
        database.write_bytes(b"business database fixture")
        database.chmod(0o600)

        preserved = {
            install_bin: (b"old executable", 0o751),
            config: (b"old config\n", 0o640),
            env_file: (b"old env\n", 0o640),
            unit: (b"old systemd unit\n", 0o644),
            dropin: (b"old drop-in\n", 0o644),
            safety_copy: (b"old helper\n", 0o644),
            wrappers[0]: (b"old wrapper\n", 0o755),
        }
        for path, (content, mode) in preserved.items():
            path.write_bytes(content)
            path.chmod(mode)
        for path in (update_bin, wrappers[1], wrappers[2]):
            self.assertFalse(path.exists())

        binary = self.root / "new-binary"
        binary.write_text("#!/bin/sh\nprintf '%s\\n' 'nexushub-webd 1.2.3'\n", encoding="utf-8")
        binary.chmod(0o755)
        next_config = self.root / "next-config"
        next_config.write_text("new config\n", encoding="utf-8")
        next_env = self.root / "next-env"
        next_env.write_text("SYNTHETIC_KEY=value\n", encoding="utf-8")
        call_log = self.root / "systemctl.log"

        assignments = {
            "INSTALL_BIN": install_bin,
            "UPDATE_BIN": update_bin,
            "SHARE_DIR": share_dir,
            "CONFIG_DIR": config_dir,
            "CONFIG_FILE": config,
            "ENV_FILE": env_file,
            "DATA_DIR": data_dir,
            "LOG_DIR": log_dir,
            "BACKUP_DIR": data_dir / "backups",
            "NGINX_BACKUP_DIR": data_dir / "backups/nginx",
            "SYSTEMD_UNIT": unit,
            "SYSTEMD_DROPIN_DIR": dropin_dir,
            "PROVIDER_PATHS_DROPIN": dropin,
            "CODEX_PRECHECK_WRAPPER_BIN": wrappers[0],
            "CODEX_UPDATE_WRAPPER_BIN": wrappers[1],
            "CODEX_PRUNE_WRAPPER_BIN": wrappers[2],
            "LEGACY_DIR": legacy_dir,
            "NGINX_SNIPPET": managed / "nginx/snippets/nexushub.conf",
            "INSTALL_CONFIG_SOURCE": next_config,
            "INSTALL_ENV_SOURCE": next_env,
            "TEST_SYSTEMCTL_LOG": call_log,
        }
        set_lines = "\n".join(f"{key}={shlex.quote(str(value))}" for key, value in assignments.items())
        harness = f"""#!/usr/bin/env bash
set -Eeuo pipefail
source {shlex.quote(str(INSTALL_PATH))}
require_root() {{ :; }}
install_dirs() {{ :; }}
retire_web_payload() {{ :; }}
install_codex_home_write_paths() {{ :; }}
install_config() {{
  copy_legacy_runtime_once
  replace_file "${{INSTALL_CONFIG_SOURCE}}" "${{CONFIG_FILE}}" 0640
  replace_file "${{INSTALL_ENV_SOURCE}}" "${{ENV_FILE}}" 0640
}}
systemctl() {{
  printf '%s\\n' "$*" >> "${{TEST_SYSTEMCTL_LOG}}"
  case "$*" in
    "is-active --quiet nexushub-webd.service") return 1 ;;
    "list-unit-files nexushub.service") return 1 ;;
    "restart nexushub-webd.service") return 77 ;;
  esac
  return 0
}}
{set_lines}
main --binary {shlex.quote(str(binary))}
"""
        harness_path = self.root / "install-failure-harness.sh"
        harness_path.write_text(harness, encoding="utf-8")
        result = subprocess.run(
            ["bash", str(harness_path)],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
            check=False,
        )

        self.assertEqual(result.returncode, 77, msg=f"stdout={result.stdout}\nstderr={result.stderr}")
        calls = call_log.read_text(encoding="utf-8").splitlines()
        self.assertIn("restart nexushub-webd.service", calls)
        for path, (content, mode) in preserved.items():
            with self.subTest(path=str(path)):
                self.assertEqual(path.read_bytes(), content)
                self.assertEqual(stat.S_IMODE(path.stat().st_mode), mode)
        for path in (update_bin, wrappers[1], wrappers[2]):
            with self.subTest(path=str(path)):
                self.assertFalse(path.exists())
        self.assertEqual(database.read_bytes(), b"business database fixture")
        self.assertEqual(stat.S_IMODE(database.stat().st_mode), 0o600)

    def test_update_download_failures_stop_before_systemd_or_installer(self) -> None:
        real_id = shutil.which("id")
        self.assertIsNotNone(real_id)
        for fail_at in (1, 2):
            with self.subTest(fail_at=fail_at):
                fixture = self.root / f"update-{fail_at}"
                fake_bin = fixture / "bin"
                fake_bin.mkdir(parents=True)
                call_log = fixture / "calls.jsonl"
                id_script = fake_bin / "id"
                id_script.write_text(
                    "#!/bin/sh\n"
                    "if [ \"${1:-}\" = \"-u\" ]; then printf '0\\n'; exit 0; fi\n"
                    f"exec {shlex.quote(str(real_id))} \"$@\"\n",
                    encoding="utf-8",
                )
                id_script.chmod(0o755)
                curl_script = fake_bin / "curl"
                curl_script.write_text(
                    f"#!{sys.executable}\n"
                    "import json, os, pathlib, sys\n"
                    "log = pathlib.Path(os.environ['TEST_FAKE_CALL_LOG'])\n"
                    "calls = log.read_text().splitlines() if log.exists() else []\n"
                    "calls.append(json.dumps(sys.argv[1:]))\n"
                    "log.write_text('\\n'.join(calls) + '\\n')\n"
                    "if len(calls) == int(os.environ['TEST_FAIL_CURL_AT']):\n"
                    "    raise SystemExit(22)\n"
                    "output = pathlib.Path(sys.argv[sys.argv.index('-o') + 1])\n"
                    "output.write_bytes(b'synthetic invalid archive')\n",
                    encoding="utf-8",
                )
                curl_script.chmod(0o755)
                for command_name in ("systemctl", "systemd-run"):
                    command = fake_bin / command_name
                    command.write_text(
                        "#!/bin/sh\n"
                        f"printf '%s\\n' {shlex.quote(command_name)} >> \"$TEST_FAKE_CALL_LOG\"\n"
                        "exit 97\n",
                        encoding="utf-8",
                    )
                    command.chmod(0o755)

                environment = os.environ.copy()
                environment.update(
                    {
                        "PATH": f"{fake_bin}{os.pathsep}{environment.get('PATH', '')}",
                        "NEXUSHUB_WEBD_UPDATE_TRANSIENT": "1",
                        "NEXUSHUB_WEBD_REPO": "example.invalid/fixture",
                        "TEST_FAKE_CALL_LOG": str(call_log),
                        "TEST_FAIL_CURL_AT": str(fail_at),
                    }
                )
                result = subprocess.run(
                    ["bash", str(UPDATE_PATH), "--version", "latest"],
                    cwd=ROOT,
                    env=environment,
                    text=True,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    timeout=30,
                    check=False,
                )

                self.assertEqual(result.returncode, 22, msg=f"stdout={result.stdout}\nstderr={result.stderr}")
                calls = [json.loads(line) for line in call_log.read_text(encoding="utf-8").splitlines()]
                self.assertEqual(len(calls), fail_at)
                self.assertTrue(all(call and call[0] == "-fL" for call in calls))
                self.assertFalse(any(call_name in str(calls) for call_name in ("systemctl", "systemd-run")))


if __name__ == "__main__":
    unittest.main(verbosity=2)
