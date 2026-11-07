#!/usr/bin/env bash
# Build deterministic first-party packages locally. Does not publish or change Git.
set -euo pipefail
repo_root="$(cd "$(dirname "$0")/.." && pwd)"
output_dir="${1:-$repo_root/target/extension-packages}"
mkdir -p "$output_dir"
for extension in review-heatmap anki-connect reviewer-progress; do
  cargo build --manifest-path "$repo_root/Cargo.toml" --locked --release \
    --target wasm32-unknown-unknown --package "$extension"
  module="${extension//-/_}"
  wasm-tools component new "$repo_root/target/wasm32-unknown-unknown/release/$module.wasm" \
    -o "$output_dir/$extension.wasm"
  wasm-tools validate --features component-model "$output_dir/$extension.wasm"
done
python3 - "$repo_root" "$output_dir" <<'PY'
import hashlib, json, sys, zipfile
from pathlib import Path
root, out = map(Path, sys.argv[1:])
for name in ('review-heatmap', 'anki-connect', 'reviewer-progress'):
    manifest = (root / 'extensions' / name / 'manifest.json').read_bytes()
    meta = json.loads(manifest)
    package = out / f'{name}-{meta["version"]}.mext'
    files = [('manifest.json', manifest), (meta.get('wasm', meta.get('entry', f'{name}.wasm')), (out / f'{name}.wasm').read_bytes()), ('LICENSE.txt', (root / 'LICENSE').read_bytes())]
    with zipfile.ZipFile(package, 'w', zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for filename, content in files:
            info = zipfile.ZipInfo(filename, (1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, content)
    print(f'{package.name}: {package.stat().st_size} bytes; sha256 {hashlib.sha256(package.read_bytes()).hexdigest()}')
print('Local packages ready. Nothing published.')
PY
