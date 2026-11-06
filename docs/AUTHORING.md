# Authoring a Manatan Flashcards extension

An extension is a WebAssembly **component** (a `.wasm` targeting the
[`extension` world](../crates/manatan-flashcards-sdk/wit/world.wit)) plus a `manifest.json`, zipped together
into a `.mext` file. This guide covers writing, building, and packaging one in
Rust with [`manatan-flashcards-sdk`](../crates/manatan-flashcards-sdk).

## 1. Toolchain

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-tools           # componentizes the module, validates the world
```

Extensions target `wasm32-unknown-unknown`: they use only the host
capabilities in the WIT world, so they need **no WASI** — no ambient clock,
random, filesystem, or sockets. Use `host::now_millis`, `host::log`, `net`,
etc. instead of `std` I/O.

## 2. Write the extension

Start from the standalone [`examples/deck-counter`](../examples/deck-counter)
project to use the published SDK: `manatan-flashcards-sdk = "=0.1.0"`. Its README
includes a packaging command and an install/permissions/lifecycle checklist.
For development inside this repository, `extensions/sample-extension` uses
`cdylib` and the workspace's local SDK dependency instead.

```rust
use manatan_flashcards_sdk::{collection, host, store, Extension};

#[derive(Default)]
struct MyExtension;

impl Extension for MyExtension {
    fn init(&mut self) {
        host::log(host::LogLevel::Info, "my-extension loaded");
    }

    // Return an HTML fragment; the host injects it into the surface.
    fn render(&mut self, surface: String, _context: String) -> String {
        if surface != "dashboard" { return String::new(); }
        format!("<p>{} decks</p>", collection::list_decks().len())
    }
}

manatan_flashcards_sdk::export_extension!(MyExtension);
```

Every [`Extension`] hook has a default, so implement only what you need:

| Hook | When |
|---|---|
| `init` | once per enable-cycle — register commands (`ui::register_command`) and timers (`tasks::set_timer`) here |
| `on_collection_loaded` | the collection opens or switches |
| `on_card_shown(card)` / `on_answer_shown(card)` | a card side is shown |
| `on_review_answered(card, button)` | the user answers (1=again … 4=easy) |
| `on_note_added(id)` / `on_notes_will_be_deleted(ids)` | notes change (the delete hook runs while they still exist) |
| `transform_card_html(card, side, html)` | rewrite a rendered card |
| `render(surface, context) -> html` | the host renders a surface; return HTML to inject |
| `on_command(id)` | the user activates a command you registered |
| `on_timer(id)` | a timer you registered fires |
| `handle_http(request) -> response` | a request arrives on a `net::serve` port |

See [`extensions/sample-extension`](../extensions/sample-extension) for a minimal
extension, and [`extensions/event-logger`](../extensions/event-logger) for hooks,
commands, and timers in one place.

## 3. Build the component

```sh
cargo build -p sample-extension --target wasm32-unknown-unknown --release

wasm-tools component new \
  target/wasm32-unknown-unknown/release/sample_extension.wasm \
  -o sample-extension.wasm
```

`wasm-tools component new` wraps the core module into a component that imports
the `manatan:srs` interfaces and exports `guest`. Validate it with:

```sh
wasm-tools validate --features component-model sample-extension.wasm
wasm-tools component wit sample-extension.wasm   # inspect the world it targets
```

## 4. The manifest

`manifest.json` sits next to the `.wasm` and declares metadata and the
permissions the host will prompt the user to grant.

```json
{
  "schemaVersion": 1,
  "id": "my-extension",
  "name": "My Extension",
  "version": "1.0.0",
  "description": "What it does.",
  "author": "you",
  "homepage": "https://example.com/optional",
  "license": "MIT",
  "apiVersion": 1,
  "wasm": "my-extension.wasm",
  "surfaces": ["dashboard"],
  "permissions": {
    "collection": "read",
    "scheduler": false,
    "reviewer": false,
    "ui": false
  }
}
```

| Field | Meaning |
|---|---|
| `id` | unique, `[a-z0-9-]`, 2–64 chars, starts with a letter |
| `apiVersion` | the WIT `host::api-version` this targets (currently `1`) |
| `wasm` | the component filename inside the package |
| `surfaces` | which `render` surfaces this contributes to: `dashboard`, `overview`, `reviewer`, `browser`, `editor`, `stats`, `deck-options` |
| `permissions.collection` | `"none"` \| `"read"` \| `"write"` — collection access |
| `permissions.scheduler` | `true` for `scheduler::*` (answer, reschedule, deck config) |
| `permissions.reviewer` | `true` for `reviewer::*` |
| `permissions.ui` | `true` to register commands/columns/editor buttons, notify, open dialogs |
| `permissions.files` | `true` for user-mediated open/save file pickers |
| `permissions.media` | `true` for audio playback, TTS, screenshot, clipboard |
| `permissions.tasks` | `true` for background timers |
| `permissions.network.allow` | URL prefixes `net::fetch` may reach (empty = no fetch) |
| `permissions.serve.ports` | localhost ports for `net::serve` (empty = no serve) |

`store` (per-extension key-value + config) and `host` (log, time, theme,
locale) are always available and need no permission. Request the minimum — the
user sees the granted list before installing. See the
[capability guide](CAPABILITIES.md) for more examples and boundaries.

The host **enforces** these: it only links the capability interfaces the
manifest requests, so calling an ungranted capability fails to instantiate.
Request the minimum you need — users see the list before installing.

## 5. Package and install

```sh
zip my-extension.mext manifest.json my-extension.wasm
```

Install it from the Manatan Flashcards app: **Extensions → Install from File**, or
publish it to the shared gallery and install via **Get Shared**. Either way
the app shows the manifest's requested permissions in a consent dialog before
anything is installed, so request the minimum you need.

Test in a compatible Manatan Flashcards host with a disposable collection before
sharing. A valid component is not proof that every host platform supports
every capability; local HTTP listeners, native file pickers, and system media
features need platform-specific testing.

## Rendering surfaces

`render(surface, context)` is called when the host builds a page. Return an
HTML fragment (styles inline or in a `<style>` block) and the host splices it
into that surface. `context` is a JSON object describing the page — for
`reviewer` it includes the current card; for `dashboard` it includes the active
collection. Return `""` to contribute nothing to a given surface.

Interactive controls in a `reviewer` fragment can call back into your
extension: the host exposes review actions (`reviewer::answer`, etc.) that your
`handle_http`/event hooks can drive. Long-running or headless work belongs in a
`serve` handler or the review-event hooks, not in `render`.

## Driving the reviewer

With the `reviewer` permission, `reviewer::current-card` reads the card under
review and `reviewer::answer(button)` / `show-answer` / `suspend-card` / `undo`
/ `set-flag` / `toggle-mark` / `replay-audio` drive it. Flag and mark changes
apply immediately; the answer/flip actions are handed to the app's reviewer,
which applies them on its next tick. This is how a Tools-menu command
(`on-command`) or a timer can control review — see
[`extensions/event-logger`](../extensions/event-logger).

## Custom field filters

With the `collection` permission, `field-filter` implements a custom
`{{myfilter:Field}}` template filter. When a note is rendered, any filter the
built-ins don't recognize is offered to your extension; return `some(text)` to
substitute, or `none` to pass. The filter runs when the card's HTML is
generated (add/edit), so re-render an existing note (edit it) to see changes.
