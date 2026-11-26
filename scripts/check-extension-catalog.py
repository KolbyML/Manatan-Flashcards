#!/usr/bin/env python3
"""Check the reviewed Flashcards listings and publisher bindings."""
import argparse
import json
from pathlib import Path
import re
from urllib.parse import urlsplit

ID = re.compile(r"[a-z][a-z0-9-]{1,63}")
KEY = re.compile(r"[0-9a-f]{64}")


def https(url):
    parsed = urlsplit(url)
    return parsed.scheme == "https" and parsed.hostname and not (
        parsed.username or parsed.password or parsed.query or parsed.fragment
    )


def validate(path):
    catalog = json.loads(path.read_bytes())
    if catalog["schemaVersion"] != 1 or not isinstance(catalog["extensions"], list):
        raise ValueError("Invalid catalog format")
    extensions = set()
    publishers = {}
    for row in catalog["extensions"]:
        extension = row["id"]
        source = row["repository"]
        publisher = source["id"]
        if not ID.fullmatch(extension) or extension in extensions:
            raise ValueError(f"Invalid or duplicate extension ID: {extension}")
        extensions.add(extension)
        for text, limit in ((row["name"], 120), (row["description"], 1000), (source["name"], 128)):
            if not text.strip() or len(text) > limit:
                raise ValueError(f"Invalid display text for {extension}")
        if not https(row["sourceUrl"]):
            raise ValueError(f"Expected an HTTPS source URL for {extension}")
        if not ID.fullmatch(publisher) or not KEY.fullmatch(source["trustedRoot"]["ed25519"]):
            raise ValueError(f"Invalid publisher or Ed25519 key for {extension}")
        for field, suffix in (("metadataUrl", "/metadata/"), ("targetsUrl", "/targets/")):
            if not https(source[field]) or not source[field].endswith(suffix):
                raise ValueError(f"Expected an HTTPS {suffix} URL for {extension}")
        if source["metadataUrl"].removesuffix("/metadata/") != source["targetsUrl"].removesuffix("/targets/"):
            raise ValueError(f"Publisher directories must share a base URL for {extension}")
        if publishers.setdefault(publisher, source) != source:
            raise ValueError(f"Conflicting entries for publisher {publisher}")
    print(f"Validated {len(extensions)} listings from {len(publishers)} publishers.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("catalog", type=Path)
    validate(parser.parse_args().catalog)
