# Deck Counter

The complete example from [Build your first Manatan Flashcards extension](https://manatan.com/blog/build-manatan-srs-extensions).
It displays the number of decks in the active collection on the dashboard.
It does not change cards or decks, register commands, or contact the network.

This is a standalone Rust project using the published `manatan-flashcards-sdk`
0.1.0, not a path dependency on the SDK source.
Copy this directory to start a project.
Keep `Cargo.lock` with it so subsequent builds use the same dependencies.

## Build

Install a current Rust toolchain, the `wasm32-unknown-unknown` target,
`wasm-tools`, and the `zip` utility. From the repository root:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-tools --locked
bash scripts/package-deck-counter.sh
```

The script builds and validates a WebAssembly component, then creates
`target/deck-counter/deck-counter-0.1.0.mext`. The archive contains only the
manifest, the component, and its MIT license, directly at the archive root.
It never publishes or uploads anything. ZIP timestamps and file permissions
are normalized for reproducible packaging with the same toolchain.

## Install and permissions

Use a disposable collection in a compatible Manatan Flashcards desktop app:

1. Open **Extensions → Install from File** and select the `.mext` file.
2. Review the consent dialog. The only requested capability is **Read collection**.
   Canceling must leave the extension uninstalled.
3. Approve installation, then return to the dashboard. **Your decks** should
   show the current collection's deck count, including any default deck.
4. Add a deck and refresh the dashboard; the count should increase by one.
5. Restart the app and confirm the panel still appears.
6. Disable the extension and refresh; the panel should disappear. Re-enable
   it to restore the panel.
7. Uninstall it; it should disappear from both Extensions and the dashboard.

The SDK is a developer preview. This example targets extension API version 1.
Check the host version and platforms you intend to support; a successful build
is not a claim of browser or mobile compatibility.

## Adapt it

Change the manifest ID before sharing your own extension. Keep the manifest's
`wasm` filename aligned with your packaged component. Request additional
permissions only when your feature needs them. If you render collection text
instead of a numeric count, escape it before inserting it into HTML.

MIT © Kolby Moroz Liebl. The package includes the repository's license.
