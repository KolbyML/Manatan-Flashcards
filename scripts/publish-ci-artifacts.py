#!/usr/bin/env python3
"""Receive the release job's packages through a restricted SSH key."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile

PACKAGE_NAME = re.compile(
    r"(review-heatmap|anki-connect|reviewer-progress)-[0-9]+\.[0-9]+\.[0-9]+\.mext"
)
MAX_PACKAGE_BYTES = 32 * 1024 * 1024


def receive_packages(stream, directory: Path):
    received = set()
    # The workflow sends a flat archive with short names. Stream each file
    # into a fresh directory and never extract archive paths or links.
    with tarfile.open(fileobj=stream, mode="r|", format=tarfile.USTAR_FORMAT) as archive:
        for entry in archive:
            name = PACKAGE_NAME.fullmatch(entry.name)
            if (
                name is None
                or name[1] in received
                or entry.type != tarfile.REGTYPE
                or not 0 < entry.size <= MAX_PACKAGE_BYTES
            ):
                raise ValueError("Invalid extension package in release archive")
            package = directory / entry.name
            with archive.extractfile(entry) as source, package.open("xb") as out:
                shutil.copyfileobj(source, out)
            with zipfile.ZipFile(package) as component:
                with component.open("manifest.json") as manifest_file:
                    body = manifest_file.read(32 * 1024 + 1)
                if len(body) > 32 * 1024:
                    raise ValueError("Extension manifest exceeds 32 KiB")
                manifest = json.loads(body)
                version = manifest["version"]
                if manifest["id"] != name[1] or package.name != f"{name[1]}-{version}.mext":
                    raise ValueError("Package name does not match its manifest")
            received.add(name[1])
    if not received:
        raise ValueError("The release archive has no extension packages")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--keys", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    command = re.fullmatch(r"publish (0|[1-9][0-9]?|100)", os.environ.get("SSH_ORIGINAL_COMMAND", ""))
    if command is None:
        parser.error("Expected publish followed by a rollout percentage")
    with tempfile.TemporaryDirectory(prefix="flashcards-release-") as directory:
        receive_packages(sys.stdin.buffer, Path(directory))
        subprocess.run(
            [
                sys.executable, str(Path(__file__).with_name("extension-repository.py")),
                "publish", "--keys", str(args.keys), "--out", str(args.out),
                "--packages", directory, "--rollout", command[1],
            ],
            check=True,
        )


if __name__ == "__main__":
    main()
