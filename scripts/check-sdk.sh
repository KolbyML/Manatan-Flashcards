#!/usr/bin/env bash
# Local verification only. This script never uploads or publishes anything.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo check --workspace --locked
cargo test --workspace --locked
cargo package -p manatan-flashcards-sdk --allow-dirty
package_files="$(cargo package -p manatan-flashcards-sdk --allow-dirty --list)"
for required in wit/world.wit README.md LICENSE src/lib.rs; do
  if ! printf '%s\n' "$package_files" | grep -Fxq "$required"; then
    echo "Missing SDK package input: $required" >&2
    exit 1
  fi
done
cmp LICENSE crates/manatan-flashcards-sdk/LICENSE
cargo build -p sample-extension --target wasm32-unknown-unknown --release --locked
wasm-tools component new target/wasm32-unknown-unknown/release/sample_extension.wasm -o target/sample-extension.wasm
wasm-tools validate --features component-model target/sample-extension.wasm
bash scripts/package-deck-counter.sh
echo 'SDK package and example components verified locally. Nothing published.'
