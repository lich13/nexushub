#!/usr/bin/env python3
"""Capture the packaged GTK window and reject an empty renderer under Xvfb."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

import gi
gi.require_version("Gdk", "3.0")
gi.require_version("GdkX11", "3.0")
from gi.repository import Gdk, GdkX11
from PIL import Image, ImageChops


def main():
    appimage, output = map(lambda value: Path(value).resolve(), sys.argv[1:])
    output.mkdir(parents=True, exist_ok=True)
    boot_log = Path.home() / ".local/state/NexusHub/logs/nexushub.log"
    boot_offset = boot_log.stat().st_size if boot_log.exists() else 0
    env = dict(os.environ, APPIMAGE_EXTRACT_AND_RUN="1", WEBKIT_DISABLE_COMPOSITING_MODE="1", WEBKIT_DISABLE_DMABUF_RENDERER="1")
    with (output / "app.log").open("w") as log:
        process = subprocess.Popen([str(appimage)], env=env, stdout=log, stderr=log, start_new_session=True)
        try:
            display = Gdk.Display.open(os.environ["DISPLAY"])
            if display is None:
                raise RuntimeError("Cannot open the Xvfb display")
            deadline = time.monotonic() + 45
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError(f"AppImage exited before acceptance: {process.returncode}")
                search = subprocess.run(["xdotool", "search", "--onlyvisible", "--name", "^NexusHub$"], capture_output=True, text=True)
                boot = None
                if boot_log.exists():
                    with boot_log.open() as history:
                        history.seek(boot_offset)
                        for line in history:
                            if " desktop_boot_probe " not in line:
                                continue
                            try:
                                boot = json.loads(line.split(" desktop_boot_probe ", 1)[1])
                                if isinstance(boot, str):
                                    boot = json.loads(boot)
                            except (ValueError, TypeError):
                                continue
                ready = isinstance(boot, dict) and all(boot.get(key) is True for key in ("desktopRuntime", "bootMounted", "hasMainShell", "hasDesktopNav")) and not boot.get("hasWebLoginGate") and not boot.get("hasVisibleLinuxHostCopy")
                for value in search.stdout.splitlines():
                    window = GdkX11.X11Window.foreign_new_for_display(display, int(value))
                    width, height = window.get_width(), window.get_height()
                    if width < 600 or height < 400:
                        continue
                    pixels = Gdk.pixbuf_get_from_window(window, 0, 0, width, height)
                    if pixels is None:
                        continue
                    screenshot = output / "window.png"
                    pixels.savev(str(screenshot), "png", [], [])
                    # Exclude the title bar; text edges must exist throughout the app body.
                    image = Image.open(screenshot).convert("RGB").crop((0, 60, width, height))
                    edges = ImageChops.difference(image, ImageChops.offset(image, 1, 0)).convert("L")
                    edge_pixels = sum(count for tone, count in enumerate(edges.histogram()) if tone >= 30)
                    colors = len(image.getcolors(width * height) or [])
                    if edge_pixels >= 2000 and colors >= 32 and ready:
                        result = {"appimage": appimage.name, "width": width, "height": height, "edgePixels": edge_pixels, "colors": colors, "nonblank": True, "boot": boot}
                        (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
                        print(json.dumps(result))
                        return
                time.sleep(0.5)
            raise RuntimeError("No nonblank NexusHub window rendered within 45 seconds; inspect window.png and app.log")
        finally:
            try:
                os.killpg(process.pid, signal.SIGTERM)
                process.wait(timeout=5)
            except ProcessLookupError:
                pass
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()


if __name__ == "__main__":
    main()
