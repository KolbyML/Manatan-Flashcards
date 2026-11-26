//! A reviewer-surface extension: shows an unobtrusive "N reviewed today" badge
//! after the current card has appeared. Older hosts use the original count.

use manatan_flashcards_sdk::{collection, host, Extension};

#[derive(Default)]
struct ReviewerProgress;

impl Extension for ReviewerProgress {
    fn render(&mut self, surface: String, context: String) -> String {
        if surface != "reviewer" {
            return String::new();
        }
        if serde_json::from_str::<serde_json::Value>(&context)
            .is_ok_and(|context| context["supportsCollectionQueries"] == true)
        {
            return include_str!("badge.html").to_owned();
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
