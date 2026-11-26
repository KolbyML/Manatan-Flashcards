#!/usr/bin/env python3
"""Sign package hashes and release metadata with one Ed25519 publisher key."""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile
import zipfile

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.serialization import load_pem_private_key

ROOT = Path(__file__).resolve().parents[1]
DOMAIN = b"manatan-flashcards-release-feed-v1\0"
OFFICIAL = {"review-heatmap", "anki-connect", "reviewer-progress"}
EXTENSION_ID = re.compile(r"[a-z][a-z0-9-]{1,63}")
VERSION = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)")


def write_file(path: Path, data: bytes):
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as output:
        temporary = Path(output.name)
        try:
            output.write(data)
            output.flush()
            temporary.chmod(0o644)
            os.replace(temporary, path)
        finally:
            temporary.unlink(missing_ok=True)


def sha256(data: bytes):
    return hashlib.sha256(data).hexdigest()


def publish(
    feed: Path, key: Ed25519PrivateKey, packages: Path | None, rollout: int,
    withdraw: list[str], publisher_public_key: Path | None = None,
):
    public_key_file = publisher_public_key or ROOT / "updates/publisher.pub"
    expected = bytes.fromhex(public_key_file.read_text().strip())
    if not isinstance(key, Ed25519PrivateKey) or key.public_key().public_bytes_raw() != expected:
        raise ValueError("Signing key does not match the publisher's public key")
    index = feed / "metadata/releases.json"
    catalog = {"schemaVersion": 1, "sequence": 0, "releases": []}
    if index.exists():
        prior = json.loads(index.read_bytes())
        payload = base64.b64decode(prior["payload"], validate=True)
        key.public_key().verify(bytes.fromhex(prior["signature"]), DOMAIN + payload)
        catalog = json.loads(payload)
        if catalog["schemaVersion"] != 1:
            raise ValueError("Unsupported release catalog schema")
    releases = {entry["name"]: entry for entry in catalog["releases"]}
    wit = (ROOT / "crates/manatan-flashcards-sdk/wit/world.wit").read_text()
    contract = sha256(re.sub(r"\s+", "", re.sub(r"//[^\n]*", "", wit)).encode())
    package_paths = sorted(packages.glob("*.mext")) if packages else []
    if not package_paths and not withdraw:
        raise ValueError("Select a package or a release to withdraw")
    for path in package_paths:
        data = path.read_bytes()
        if not 0 < len(data) <= 32 * 1024 * 1024:
            raise ValueError("Invalid package size")
        with zipfile.ZipFile(path) as archive:
            manifest = json.loads(archive.read("manifest.json"))
        extension, version = manifest["id"], manifest["version"]
        if not EXTENSION_ID.fullmatch(extension) or not VERSION.fullmatch(version):
            raise ValueError("Expected an extension ID and a stable semantic version")
        if publisher_public_key is None and extension not in OFFICIAL:
            raise ValueError("Only official extensions can use the default Manatan publisher")
        name = f"{extension}-{version}.mext"
        if path.name != name:
            raise ValueError("Package filename does not match its manifest")
        checksum = sha256(data)
        old = releases.get(name)
        if old and old["hash"] != checksum:
            raise ValueError(f"{name} is immutable; publish a new version")
        target = feed / "targets" / f"{checksum}.{name}"
        if target.exists() and target.read_bytes() != data:
            raise ValueError("Existing package bytes do not match the release")
        if not target.exists():
            write_file(target, data)
        releases[name] = {
            "name": name, "hash": checksum, "length": len(data),
            "release": {
                "id": extension, "version": version,
                "title": manifest.get("name") or extension,
                "description": manifest.get("description") or "",
                "apiVersion": manifest.get("apiVersion", 1),
                "stateVersion": manifest.get("stateVersion", 1),
                "witSha256": contract, "channel": "stable", "rollout": rollout,
                "withdrawn": old["release"]["withdrawn"] if old else False,
            },
        }
    for name in withdraw:
        if name not in releases:
            raise ValueError(f"Unknown release: {name}")
        releases[name]["release"]["withdrawn"] = True
    if len(releases) > 1000:
        raise ValueError("Too many releases in the catalog")
    catalog["sequence"] += 1
    catalog["releases"] = [releases[name] for name in sorted(releases)]
    payload = json.dumps(catalog, separators=(",", ":")).encode()
    write_file(index, json.dumps({
        "payload": base64.b64encode(payload).decode(),
        "signature": key.sign(DOMAIN + payload).hex(),
    }, separators=(",", ":")).encode())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--key", required=True, type=Path)
    parser.add_argument(
        "--publisher-public-key", type=Path,
        help="A raw hexadecimal Ed25519 .pub file for a third-party feed",
    )
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--packages", type=Path)
    parser.add_argument("--rollout", type=int, choices=range(101), default=100)
    parser.add_argument("--publisher-id", help="Publisher ID shown by Add repository")
    parser.add_argument("--publisher-name", help="Publisher name shown in the app")
    parser.add_argument("--withdraw", action="append", default=[])
    args = parser.parse_args()
    key = load_pem_private_key(args.key.read_bytes(), password=None)
    if (args.publisher_id is None) != (args.publisher_name is None):
        parser.error("Provide both --publisher-id and --publisher-name")
    if args.publisher_id is not None and (
        not EXTENSION_ID.fullmatch(args.publisher_id)
        or not args.publisher_name.strip()
        or len(args.publisher_name) > 128
    ):
        parser.error("Invalid publisher identity")
    publish(args.out, key, args.packages, args.rollout, args.withdraw, args.publisher_public_key)
    if args.publisher_id is not None:
        write_file(args.out / "metadata/repo.json", json.dumps({
            "id": args.publisher_id, "name": args.publisher_name,
            "publicKey": key.public_key().public_bytes_raw().hex(),
        }, separators=(",", ":")).encode())


if __name__ == "__main__":
    main()
