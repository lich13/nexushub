#!/usr/bin/env python3
"""Render the managed systemd drop-in for custom Grok and Pi session roots."""

from __future__ import annotations

import os
from pathlib import Path
import shlex
import sys


DEFAULT_ROOTS = {
    Path("/root/.grok/sessions"),
    Path("/root/.pi/agent/sessions"),
}
FORBIDDEN_ROOTS = {
    Path("/"),
    Path("/root"),
    Path("/home"),
    Path("/etc"),
    Path("/usr"),
    Path("/var"),
}
SUPPORTED_KEYS = {
    "GROK_HOME",
    "PI_CODING_AGENT_DIR",
    "PI_CODING_AGENT_SESSION_DIR",
}


def parse_environment_file(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    if not path.exists():
        return values
    for number, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, encoded = line.split("=", 1)
        key = key.strip()
        if key not in SUPPORTED_KEYS:
            continue
        lexer = shlex.shlex(encoded, posix=True)
        lexer.whitespace_split = True
        lexer.commenters = "#"
        tokens = list(lexer)
        if len(tokens) != 1 or not tokens[0]:
            raise ValueError(f"{path}:{number}: {key} must contain one non-empty path")
        values[key] = tokens[0]
    return values


def normalize_session_root(value: str, source: str) -> Path:
    if any(ord(character) < 32 for character in value):
        raise ValueError(f"{source} contains a control character")
    path = Path(os.path.normpath(value))
    if not path.is_absolute():
        raise ValueError(f"{source} must be an absolute path")
    if path in FORBIDDEN_ROOTS:
        raise ValueError(f"{source} is too broad for ReadWritePaths: {path}")
    if path in DEFAULT_ROOTS:
        return path
    current = Path("/")
    for part in path.parts[1:]:
        current /= part
        if current.exists() and current.is_symlink():
            raise ValueError(f"{source} traverses a symlink: {current}")
    return path


def custom_session_roots(values: dict[str, str]) -> list[Path]:
    roots: list[Path] = []
    if grok_home := values.get("GROK_HOME"):
        roots.append(normalize_session_root(str(Path(grok_home) / "sessions"), "GROK_HOME"))
    if pi_sessions := values.get("PI_CODING_AGENT_SESSION_DIR"):
        roots.append(normalize_session_root(pi_sessions, "PI_CODING_AGENT_SESSION_DIR"))
    elif pi_agent := values.get("PI_CODING_AGENT_DIR"):
        roots.append(
            normalize_session_root(
                str(Path(pi_agent) / "sessions"),
                "PI_CODING_AGENT_DIR",
            )
        )
    return sorted(set(roots) - DEFAULT_ROOTS, key=str)


def systemd_quote(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def render(values: dict[str, str]) -> str:
    roots = custom_session_roots(values)
    if not roots:
        return ""
    lines = ["[Service]"]
    lines.extend(f"ReadWritePaths={systemd_quote('-' + str(root))}" for root in roots)
    return "\n".join(lines) + "\n"


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: provider-session-write-paths.py ENV_FILE OUTPUT", file=sys.stderr)
        return 2
    env_path = Path(sys.argv[1])
    output_path = Path(sys.argv[2])
    try:
        output = render(parse_environment_file(env_path))
    except (OSError, ValueError) as error:
        print(f"provider session write path error: {error}", file=sys.stderr)
        return 1
    output_path.write_text(output, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
