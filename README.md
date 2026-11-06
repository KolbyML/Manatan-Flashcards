# Manatan Flashcards — Extension SDK

Public SDK for building [Manatan Flashcards](https://manatan.com/srs) extensions as
sandboxed **WebAssembly components**.

Manatan Flashcards is a spaced-repetition app. Build dashboard visualizations, review
tools, custom study workflows, and local integrations as self-contained
WebAssembly packages with explicit permissions.

The [SDK 0.1.0 developer preview](https://crates.io/crates/manatan-flashcards-sdk/0.1.0)
is available on crates.io. Standalone projects can use:

```toml
manatan-flashcards-sdk = "=0.1.0"
```

Use the local workspace dependency when developing this repository. The
extension protocol remains `manatan:srs`, preserving existing host compatibility.

- **Independent of the private host.** This repo defines the entire contract in
  [`world.wit`](crates/manatan-flashcards-sdk/wit/world.wit) and a thin Rust SDK over it. Nothing here
  depends on Manatan's closed-source host. Anyone can build an extension
  without access to, or changes to, the Manatan app.
- **Capability-sandboxed.** An extension can only touch what it declares in its
  manifest and the user grants: the collection, the reviewer, key-value
  storage, the network, an HTTP listener. No ambient filesystem or network
  access — the host mediates every call.
- **Language-agnostic contract.** The SDK here is Rust, but the WIT world is
  the real API; any language that targets the Component Model can implement it.

## Quickstart

```rust
use manatan_flashcards_sdk::{collection, Extension};

#[derive(Default)]
struct DeckCounter;

impl Extension for DeckCounter {
    fn render(&mut self, surface: String, _ctx: String) -> String {
        if surface != "dashboard" { return String::new(); }
        format!("<p>You have {} decks.</p>", collection::list_decks().len())
    }
}

manatan_flashcards_sdk::export_extension!(DeckCounter);
```

Then build it into a component and zip it with a `manifest.json` — see
[`docs/AUTHORING.md`](docs/AUTHORING.md).

For a complete standalone project and a local `.mext` packaging command, see
the [Deck Counter tutorial example](examples/deck-counter). It builds against
the published SDK and requests only read access to the collection.

## What an extension can do

See [the capability guide](docs/CAPABILITIES.md) for the available interfaces,
examples, and sandbox boundaries. The contract describes the extension API;
individual host versions and platforms can have different capabilities.

| Interface | Powers |
|---|---|
| `collection` | search; read/write notes, cards, decks, note types, tags; find-and-replace; **review history & forecast**; media |
| `scheduler` | answer, reschedule, bury/suspend, flags, deck options, custom scheduling |
| `reviewer` | current card, show answer, answer, bury/suspend, flag, mark, replay, undo, edit note |
| `ui` | menu/toolbar/shortcut commands, browser columns, editor buttons, notifications, dialogs, selection |
| `files` | user-mediated open/save (import/export — no ambient filesystem) |
| `media` | audio playback, text-to-speech, screenshot, clipboard |
| `net` | allowlisted `fetch` and host-managed local HTTP listeners |
| `store` | per-extension key-value + user-editable config |
| `tasks` | background timers |
| `host` | logging, time, theme, locale, API version |

Extensions react to ~30 lifecycle/event hooks (review, note/card data, editor,
browser, sync…), render HTML into **surfaces** (`dashboard`, `overview`,
`reviewer`, `browser`, `editor`, `stats`), transform card HTML, and implement
custom `{{filter:field}}` template filters.

## Layout

```
crates/manatan-flashcards-sdk/wit/       the public contract (included in the crate)
crates/manatan-flashcards-sdk/           ergonomic Rust bindings over the WIT
extensions/sample-extension/      a minimal, buildable extension
extensions/review-heatmap/        contribution-calendar on the dashboard surface
extensions/reviewer-progress/     a "N reviewed today" badge on the reviewer surface
extensions/power-extension/       a Tools-menu command that buries the current card
extensions/anki-connect/          compatibility bridge for existing external tools
extensions/event-logger/          hooks, a Tools-menu command, and a timer
examples/deck-counter/            standalone, registry-SDK tutorial example
docs/AUTHORING.md                 toolchain, build, manifest, packaging
docs/RELEASING.md                 local verification and approval gates
```

## License

MIT © Kolby Moroz Liebl. See [LICENSE](LICENSE).
