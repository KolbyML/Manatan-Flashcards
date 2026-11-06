use manatan_flashcards_sdk::{collection, Extension};

#[derive(Default)]
struct DeckCounter;

impl Extension for DeckCounter {
    fn render(&mut self, surface: String, _context: String) -> String {
        if surface != "dashboard" {
            return String::new();
        }

        let count = collection::list_decks().len();
        format!("<section><h3>Your decks</h3><p>Decks in this collection: {count}</p></section>")
    }
}

manatan_flashcards_sdk::export_extension!(DeckCounter);
