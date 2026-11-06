//! A minimal Manatan SRS extension and a starting point for your own.
//!
//! It adds a small panel to the dashboard that reports how many decks you have
//! and how many times the panel has rendered (persisted via the `store`
//! capability). It touches three host capabilities — `host`, `collection`,
//! `store` — and nothing else, so its manifest requests only those.

use manatan_flashcards_sdk::{collection, host, store, Extension};

#[derive(Default)]
struct SampleExtension;

impl Extension for SampleExtension {
    fn init(&mut self) {
        host::log(host::LogLevel::Info, "sample-extension loaded");
    }

    fn render(&mut self, surface: String, _context: String) -> String {
        // Only contribute to the dashboard surface.
        if surface != "dashboard" {
            return String::new();
        }

        let deck_count = collection::list_decks().len();

        // Persist a render counter across restarts via per-extension storage.
        let renders = store::get("renders")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
            + 1;
        store::set("renders", &renders.to_string());

        format!(
            "<section class=\"srs-ext-card\">\
               <h3>Sample extension</h3>\
               <p>You have {deck_count} deck(s).</p>\
               <p>This panel has rendered {renders} time(s).</p>\
             </section>"
        )
    }
}

manatan_flashcards_sdk::export_extension!(SampleExtension);
