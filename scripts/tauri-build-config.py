#!/usr/bin/env python3
"""Prepare one Tauri build hook and validate explicitly reused frontend output."""
import json
import sys
from html.parser import HTMLParser
from pathlib import Path


class AssetParser(HTMLParser):
    def __init__(self):
        super().__init__()
        self.assets = []

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == "script" and "src" in attrs:
            self.assets.append(attrs["src"])
        if tag == "link" and attrs.get("rel") in ("stylesheet", "modulepreload"):
            self.assets.append(attrs.get("href", ""))


def prepare(source, output, webui, signed, skip):
    config = json.loads(Path(source).read_text())
    root = (Path(webui) / "dist").resolve()
    build = config.setdefault("build", {})
    build["frontendDist"] = str(root)
    if Path(webui).resolve() != (Path(source).resolve().parent.parent / "webui").resolve():
        build["beforeBuildCommand"] = {"script": "corepack pnpm@11.0.8 build:tauri", "cwd": str(Path(webui).resolve())}
    if not signed:
        config.setdefault("bundle", {})["createUpdaterArtifacts"] = False
        config.get("plugins", {}).pop("updater", None)
    if skip:
        parser = AssetParser()
        parser.feed((root / "index.html").read_text())
        if not parser.assets:
            raise ValueError("reused Tauri frontend has no assets")
        for asset in parser.assets:
            path = (root / asset).resolve()
            if not asset.startswith("./") or not path.is_relative_to(root) or not path.is_file():
                raise ValueError(f"reused Tauri frontend requires existing relative assets: {asset}")
        build["beforeBuildCommand"] = ""
    Path(output).write_text(json.dumps(config, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    source, output, webui, signed, skip = sys.argv[1:]
    prepare(source, output, webui, signed == "1", skip == "1")
