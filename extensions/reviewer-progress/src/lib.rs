//! A reviewer-surface extension: shows an unobtrusive "N reviewed today" badge
//! on the card page. Demonstrates rendering into the reviewer surface from the
//! generic `collection.review-log` capability.

use manatan_flashcards_sdk::{collection, host, Extension};

#[derive(Default)]
struct ReviewerProgress;

impl Extension for ReviewerProgress {
    fn render(&mut self, surface: String, _context: String) -> String {
        if surface != "reviewer" {
            return String::new();
        }
        let now_ms = host::now_millis();
        let count = collection::review_log(now_ms - 86_400_000, now_ms).len();
        format!(
            "<div style=\"position:fixed;bottom:8px;right:8px;padding:4px 8px;\
             border-radius:12px;background:rgba(0,0,0,0.6);color:#fff;font-size:12px;\
             font-family:sans-serif;z-index:9999\">{count} reviewed today</div>"
        )
    }
}

manatan_flashcards_sdk::export_extension!(ReviewerProgress);
