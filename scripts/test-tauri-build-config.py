#!/usr/bin/env python3
"""Exercise signed/unsigned hooks and reject server assets in skip mode."""
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("build_config", Path(__file__).with_name("tauri-build-config.py"))
build_config = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build_config)


class BuildConfigTests(unittest.TestCase):
    def test_hook_and_reuse_contract(self):
        with tempfile.TemporaryDirectory(prefix="nexushub-build-config-") as directory:
            root = Path(directory).resolve()
            source, output = root / "source.json", root / "output.json"
            original = {"build": {"beforeBuildCommand": "pnpm build:tauri", "frontendDist": "../wrong-server/dist"}, "bundle": {"createUpdaterArtifacts": True}, "plugins": {"updater": {"pubkey": "fixture"}}}
            source.write_text(json.dumps(original))
            assets = root / "dist" / "assets"
            assets.mkdir(parents=True)
            (assets / "main.js").write_text("export default 1")
            index = root / "dist" / "index.html"
            index.write_text('<script type="module" src="./assets/main.js"></script>')
            for signed in (True, False):
                for skip in (True, False):
                    with self.subTest(signed=signed, skip=skip):
                        build_config.prepare(source, output, root, signed, skip)
                        result = json.loads(output.read_text())
                        self.assertEqual(result["build"]["beforeBuildCommand"], "" if skip else {"script": "corepack pnpm@11.0.8 build:tauri", "cwd": str(root)})
                        self.assertEqual(Path(result["build"]["frontendDist"]), assets.parent)
                        self.assertEqual("updater" in result["plugins"], signed)
                        self.assertEqual(result["bundle"]["createUpdaterArtifacts"], signed)
                        self.assertEqual(json.loads(source.read_text()), original)
            for asset in ("/nexushub/assets/main.js", "/assets/main.js", "./assets/missing.js", "./../source.json", "https://example.com/main.js"):
                with self.subTest(asset=asset):
                    index.write_text(f'<script src="{asset}"></script>')
                    with self.assertRaises(ValueError):
                        build_config.prepare(source, output, root, True, True)
            index.write_text("<html></html>")
            with self.assertRaises(ValueError):
                build_config.prepare(source, output, root, True, True)
            index.unlink()
            with self.assertRaises(FileNotFoundError):
                build_config.prepare(source, output, root, True, True)


if __name__ == "__main__":
    unittest.main()
