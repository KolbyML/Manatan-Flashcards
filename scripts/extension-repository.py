#!/usr/bin/env python3
"""Build an immutable TUF repository for Manatan SDK extension packages.

Private keys are accepted only from an operator-owned directory outside this
checkout. Publishing produces static files; it never uploads or deploys them.
"""
from __future__ import annotations
import argparse
import fcntl
from datetime import datetime, timedelta, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import zipfile

from cryptography.hazmat.primitives.serialization import load_pem_private_key
from securesystemslib.signer import CryptoSigner
from tuf.api.metadata import Metadata, Root, Targets, TargetFile, MetaFile, Snapshot, Timestamp

SDK = Path(__file__).resolve().parents[1]


def atomic(path: Path, data: bytes, private: bool = False):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + '.tmp')
    with temporary.open('wb') as stream:
        if private:
            os.fchmod(stream.fileno(), 0o600)
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)


def signer(keys: Path, filename: str):
    return CryptoSigner(load_pem_private_key((keys / filename).read_bytes(), password=None))


def generate(keys: Path, filename: str):
    path = keys / filename
    if path.exists():
        raise ValueError(f'Refusing to replace a signing key: {path}')
    key = CryptoSigner.generate_ed25519()
    atomic(path, key.private_bytes, private=True)
    return key


def write_root(root: Metadata, out: Path):
    data = root.to_bytes()
    atomic(out / 'metadata' / f'{root.signed.version}.root.json', data)
    atomic(out / 'metadata' / 'root.json', data)


def initialize(keys: Path, out: Path):
    if (out / 'metadata/root.json').exists():
        raise ValueError('Repository already initialized')
    keys.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(keys, 0o700)
    root = Metadata(Root(version=1, expires=datetime.now(timezone.utc) + timedelta(days=3650)))
    roots = []
    for index in range(3):
        filename = f'root-1-{index}.pem'
        key = generate(keys, filename)
        root.signed.add_key(key.public_key, 'root')
        roots.append(filename)
    root.signed.roles['root'].threshold = 2
    for role in ['targets', 'snapshot', 'timestamp']:
        root.signed.add_key(generate(keys, f'{role}.pem').public_key, role)
    for filename in roots:
        root.sign(signer(keys, filename), append=True)
    atomic(keys / 'roots.json', json.dumps(roots).encode(), private=True)
    write_root(root, out)


def rotate(keys: Path, out: Path):
    old = Metadata.from_file(str(out / 'metadata/root.json'))
    root = Metadata.from_bytes(old.to_bytes())
    root.signed.version += 1
    root.signed.expires = datetime.now(timezone.utc) + timedelta(days=3650)
    for keyid in list(root.signed.roles['root'].keyids):
        root.signed.revoke_key(keyid, 'root')
    roots = []
    for index in range(3):
        filename = f'root-{root.signed.version}-{index}.pem'
        root.signed.add_key((signer(keys, filename) if (keys / filename).exists() else generate(keys, filename)).public_key, 'root')
        roots.append(filename)
    root.signatures.clear()
    for filename in json.loads((keys / 'roots.json').read_text()) + roots:
        if (keys / filename).exists():
            root.sign(signer(keys, filename), append=True)
    old.signed.verify_delegate('root', root.signed_bytes, root.signatures)
    root.signed.verify_delegate('root', root.signed_bytes, root.signatures)
    write_root(root, out)
    atomic(keys / 'roots.json', json.dumps(roots).encode(), private=True)


def publish(keys: Path, out: Path, packages: Path | None, withdrawals: list[str], rollout: int):
    root = Metadata.from_file(str(out / 'metadata/root.json'))
    existing = out / 'metadata/targets.json'
    old = Metadata.from_file(str(existing)) if existing.exists() else None
    targets = dict(old.signed.targets) if old else {}
    wit = (SDK / 'crates/manatan-flashcards-sdk/wit/world.wit').read_text()
    contract = re.sub(r'\s+', '', re.sub(r'//[^\n]*', '', wit))
    wit_hash = hashlib.sha256(contract.encode()).hexdigest()
    for path in sorted(packages.glob('*.mext')) if packages else []:
        data = path.read_bytes()
        if len(data) > 32 * 1024 * 1024:
            raise ValueError('Package exceeds 32 MiB')
        with zipfile.ZipFile(path) as archive:
            manifest = json.loads(archive.read('manifest.json'))
        extension_id, version = manifest['id'], manifest['version']
        if not re.fullmatch(r'[a-z][a-z0-9-]{1,63}', extension_id):
            raise ValueError('Invalid extension ID')
        if not re.fullmatch(r'\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?', version):
            raise ValueError('Extension releases require semantic versions')
        name = f'{extension_id}-{version}.mext'
        target = TargetFile.from_data(name, data, ['sha256'])
        if name in targets and targets[name].hashes != target.hashes:
            raise ValueError(f'{name} is immutable; publish a new version')
        target.unrecognized_fields['custom'] = {'manatan': {
            'id': extension_id, 'version': version, 'apiVersion': manifest.get('apiVersion', 1),
            'stateVersion': manifest.get('stateVersion', 1), 'witSha256': wit_hash,
            'channel': 'stable', 'rollout': rollout,
            'withdrawn': targets.get(name).unrecognized_fields.get('custom', {}).get('manatan', {}).get('withdrawn', False) if name in targets else False,
        }}
        targets[name] = target
        destination = out / 'targets' / f'{target.hashes["sha256"]}.{name}'
        if destination.exists() and destination.read_bytes() != data:
            raise ValueError('Existing target content differs')
        if not destination.exists():
            atomic(destination, data)
    for name in withdrawals:
        if name not in targets:
            raise ValueError(f'Unknown withdrawal target: {name}')
        targets[name].unrecognized_fields['custom']['manatan']['withdrawn'] = True
    version = (old.signed.version + 1) if old else 1
    expires = datetime.now(timezone.utc) + timedelta(days=7)
    metadata = Metadata(Targets(version=version, expires=expires + timedelta(days=83), targets=targets))
    metadata.sign(signer(keys, 'targets.pem'))
    root.signed.verify_delegate('targets', metadata.signed_bytes, metadata.signatures)
    target_bytes = metadata.to_bytes()
    snapshot = Metadata(Snapshot(version=version, expires=expires, meta={'targets.json': MetaFile(version, len(target_bytes), {'sha256': hashlib.sha256(target_bytes).hexdigest()})}))
    snapshot.sign(signer(keys, 'snapshot.pem'))
    root.signed.verify_delegate('snapshot', snapshot.signed_bytes, snapshot.signatures)
    snapshot_bytes = snapshot.to_bytes()
    timestamp = Metadata(Timestamp(version=version, expires=expires, snapshot_meta=MetaFile(version, len(snapshot_bytes), {'sha256': hashlib.sha256(snapshot_bytes).hexdigest()})))
    timestamp.sign(signer(keys, 'timestamp.pem'))
    root.signed.verify_delegate('timestamp', timestamp.signed_bytes, timestamp.signatures)
    # Timestamp is the commit point. Keep all old versioned metadata and targets
    # so in-flight and old compatible clients can finish using their snapshot.
    for name, data in [(f'{version}.targets.json', target_bytes), (f'{version}.snapshot.json', snapshot_bytes), ('targets.json', target_bytes), ('snapshot.json', snapshot_bytes), ('timestamp.json', timestamp.to_bytes())]:
        atomic(out / 'metadata' / name, data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['init', 'publish', 'rotate-root'])
    parser.add_argument('--keys', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--packages', type=Path)
    parser.add_argument('--withdraw', action='append', default=[], help='Immutable target filename to withdraw')
    parser.add_argument('--rollout', type=int, default=100)
    args = parser.parse_args()
    keys, out = args.keys.resolve(), args.out.resolve()
    if keys == SDK or SDK in keys.parents or keys == out or out in keys.parents:
        parser.error('Private keys must be outside the SDK checkout and public output directory')
    if not 0 <= args.rollout <= 100:
        parser.error('Rollout must be 0..100')
    # Timer refreshes and release publishing share one persistent counter.
    # Serialize CLI writers before reading metadata or creating temp files.
    out.mkdir(parents=True, exist_ok=True)
    with (out / '.publish.lock').open('a') as lock:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        if args.action == 'init': initialize(keys, out)
        elif args.action == 'rotate-root': rotate(keys, out)
        else: publish(keys, out, args.packages, args.withdraw, args.rollout)
    print(f'{args.action}: public repository written to {out}')


if __name__ == '__main__':
    main()
