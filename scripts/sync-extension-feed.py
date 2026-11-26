#!/usr/bin/env python3
"""Mirror the public extension-feed branch into the website's feed directory."""
import argparse
import base64
import fcntl
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

PUBLIC_NAME = re.compile(r"[a-zA-Z0-9][a-zA-Z0-9._-]{0,255}")


def copy_public_file(source: Path, destination: Path):
    if source.is_symlink() or not source.is_file() or not PUBLIC_NAME.fullmatch(source.name):
        raise ValueError(f"Invalid public feed file: {source.name}")
    with tempfile.NamedTemporaryFile(dir=destination.parent, prefix=".mirror-", delete=False) as out:
        temporary = Path(out.name)
        try:
            with source.open("rb") as stream:
                shutil.copyfileobj(stream, out)
            out.flush()
            os.fsync(out.fileno())
            temporary.chmod(0o644)
            os.replace(temporary, destination)
        finally:
            temporary.unlink(missing_ok=True)


def version(file: Path):
    envelope = json.loads(file.read_bytes())
    return json.loads(base64.b64decode(envelope["payload"], validate=True))["sequence"]


def mirror(destination: Path, remote: str):
    for kind in ("metadata", "targets"):
        (destination / kind).mkdir(parents=True, exist_ok=True)

    with (destination / ".mirror.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        with tempfile.TemporaryDirectory(prefix="flashcards-feed-") as directory:
            feed = Path(directory) / "feed"
            subprocess.run(
                ["git", "clone", "--quiet", "--branch", "extension-feed",
                 "--single-branch", "--depth", "1", "--", remote, str(feed)],
                check=True,
            )
            index = feed / "metadata/releases.json"
            current = destination / "metadata/releases.json"
            incoming_version = version(index)
            if current.exists():
                if incoming_version < version(current):
                    raise ValueError("The incoming index is older than the public feed")
                if incoming_version == version(current):
                    if index.read_bytes() != current.read_bytes():
                        raise ValueError("The incoming index reuses the public version")
                    return

            # Publish all packages before replacing the catalog. Leave any
            # legacy TUF files on the website for previously released clients.
            for source in sorted((feed / "targets").glob("*.mext")):
                copy_public_file(source, destination / "targets" / source.name)
            copy_public_file(index, current)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--remote", default="https://github.com/KolbyML/Manatan-Flashcards.git")
    args = parser.parse_args()
    mirror(args.destination, args.remote)


if __name__ == "__main__":
    main()
