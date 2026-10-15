#!/usr/bin/env python3
"""Internal installer primitives; operate only on an explicitly owned transaction."""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import sys
import tarfile
import tempfile


def extract(archive, destination):
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    if destination.is_symlink() or any(destination.iterdir()):
        raise ValueError("extraction requires an empty owned directory")
    with tarfile.open(archive, "r:gz") as source:
        members = source.getmembers()
        seen = set()
        for member in members:
            path = PurePosixPath(member.name)
            if (path.is_absolute() or ".." in path.parts or not path.parts
                    or "\\" in member.name or member.name in seen
                    or not (member.isdir() or member.isfile())):
                raise ValueError("unsafe archive member")
            seen.add(member.name)
        # Do not allow a regular member to masquerade as a parent directory.
        files = {PurePosixPath(m.name) for m in members if m.isfile()}
        if any(any(parent in files for parent in PurePosixPath(m.name).parents) for m in members):
            raise ValueError("archive file is also a parent directory")
        for member in members:
            target = destination / member.name
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                with source.extractfile(member) as incoming, target.open("xb") as output:
                    shutil.copyfileobj(incoming, output)
                target.chmod(member.mode & 0o777)


def verify(archive, checksum):
    archive = Path(archive)
    lines = Path(checksum).read_text().splitlines()
    if len(lines) != 1:
        raise ValueError("expected one release checksum")
    parts = lines[0].split()
    if len(parts) != 2 or parts[1].lstrip("*") != archive.name:
        raise ValueError("checksum does not name the downloaded archive")
    with archive.open("rb") as stream:
        digest = hashlib.sha256()
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
        actual = digest.hexdigest()
    if actual != parts[0].lower():
        raise ValueError("release checksum mismatch")


def atomic_copy(source, target, mode=None):
    target = Path(target)
    if target.is_symlink() or (target.exists() and not target.is_file()):
        raise ValueError("managed target must be a regular file")
    descriptor, temporary = tempfile.mkstemp(prefix=".nexushub-install-", dir=target.parent)
    os.close(descriptor)
    try:
        shutil.copy2(source, temporary)
        if mode is not None:
            os.chmod(temporary, int(mode, 8))
        else:
            stat = Path(source).stat()
            if os.geteuid() == 0:
                os.chown(temporary, stat.st_uid, stat.st_gid)
        with open(temporary, "rb") as stream:
            os.fsync(stream.fileno())
        os.replace(temporary, target)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def snapshot(transaction, target):
    transaction, target = Path(transaction), Path(target)
    manifest = transaction / "files.json"
    entries = json.loads(manifest.read_text()) if manifest.exists() else []
    if str(target) in [entry["target"] for entry in entries]:
        return
    if target.is_symlink() or (target.exists() and not target.is_file()):
        raise ValueError("managed target must be a regular file")
    backup = str(len(entries)) if target.exists() else None
    if backup is not None:
        shutil.copy2(target, transaction / backup)
        if os.geteuid() == 0:
            stat = target.stat()
            os.chown(transaction / backup, stat.st_uid, stat.st_gid)
    entries.append({"target": str(target), "backup": backup})
    temporary = transaction / "files.next"
    temporary.write_text(json.dumps(entries))
    os.replace(temporary, manifest)


def rollback(transaction):
    transaction = Path(transaction)
    manifest = transaction / "files.json"
    if not manifest.exists():
        return
    entries = json.loads(manifest.read_text())
    for entry in reversed(entries):
        target = Path(entry["target"])
        if entry["backup"] is None:
            if target.is_symlink() or (target.exists() and not target.is_file()):
                raise ValueError("managed target identity changed")
            target.unlink(missing_ok=True)
        else:
            atomic_copy(transaction / entry["backup"], target)


def main():
    operation, *args = sys.argv[1:]
    if operation == "extract":
        extract(*args)
    elif operation == "verify":
        verify(*args)
    elif operation == "snapshot":
        snapshot(*args)
    elif operation == "replace":
        transaction, source, target, mode = args
        snapshot(transaction, target)
        atomic_copy(source, target, mode)
    elif operation == "rollback":
        rollback(*args)
    else:
        raise ValueError("unknown installer operation")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, tarfile.TarError, EOFError, KeyError, TypeError):
        raise SystemExit("installer file validation or replacement failed") from None
