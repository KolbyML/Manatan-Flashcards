# Preparing an SDK release

The `manatan-flashcards-sdk` 0.1.0 developer preview is published on crates.io.
The original `manatan-srs-sdk` 0.1.0 remains available for existing consumers.
The standalone example uses the renamed published package directly.
Keep the `manatan:srs` WIT protocol unchanged.
Future SDK releases and extension publications require the repository owner's
explicit approval. Building a local package
does not publish it, reserve its name, or create a GitHub release.

## Local verification

```sh
cargo check --workspace --locked
cargo test --workspace --locked
cargo package -p manatan-flashcards-sdk --allow-dirty
cargo build -p sample-extension --target wasm32-unknown-unknown --release --locked
wasm-tools component new target/wasm32-unknown-unknown/release/sample_extension.wasm -o target/sample-extension.wasm
wasm-tools validate --features component-model target/sample-extension.wasm
bash scripts/package-deck-counter.sh
```

Inspect `cargo package -p manatan-flashcards-sdk --list --allow-dirty`: it must contain
`wit/world.wit`, `README.md`, `LICENSE`, and the Rust source. Verification must
build the extracted package, not just the workspace. Do not skip verification.
The local dirty flag allows review without committing the owner's work;
release provenance must be reviewed before an actual upload.

## Before requesting approval

Test the packaged example in the intended Manatan Flashcards release: install,
permission consent, render, restart, disable, and uninstall. Check both allowed
and denied capabilities and the intended platforms. Compare the component's
WIT with the host's exact contract. Preserve compatibility protocol names and
third-party credits. Review package contents and license notices.

Confirm registry account ownership, name availability, and permissions without
printing credentials. Request approval separately for the registry upload,
repository push or release, website deployment, and blog publication. Do not
put automatic publication in the build or test workflow.

## After approval

Use the normal authenticated Cargo release workflow for the approved package
and version. Confirm the public package builds before changing examples to
registry dependencies or announcing availability. Upload the reviewed blog
draft only after its own publication approval.
