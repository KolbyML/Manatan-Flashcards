//! Event Logger: records lifecycle/review hooks into the extension config
//! (readable from the app's extension settings), stamps rendered cards,
//! registers a Tools-menu command, and schedules a one-shot timer. A
//! demonstration of hooks for review alarms, streak trackers, and auto-taggers.

use manatan_flashcards_sdk::{reviewer, store, tasks, ui, Card, Extension};
use serde_json::{json, Value};

#[derive(Default)]
struct EventLogger;

/// Merge one key into the extension config, preserving the rest.
fn record(key: &str, value: Value) {
    let mut config: Value =
        serde_json::from_str(&store::config()).unwrap_or_else(|_| json!({}));
    if !config.is_object() {
        config = json!({});
    }
    config[key] = value;
    store::set_config(&config.to_string());
}

impl Extension for EventLogger {
    fn init(&mut self) {
        ui::register_command(&ui::Command {
            id: "event-logger.ping".into(),
            label: "Ping Event Logger".into(),
            location: ui::CommandLocation::ToolsMenu,
            shortcut: None,
        });
        ui::register_command(&ui::Command {
            id: "event-logger.answer".into(),
            label: "Answer current card Good".into(),
            location: ui::CommandLocation::ReviewerToolbar,
            shortcut: None,
        });
        tasks::set_timer("tick", 1_000, false);
    }

    fn on_command(&mut self, command_id: String) {
        match command_id.as_str() {
            "event-logger.ping" => {
                record("pinged", json!(true));
                ui::notify("pong", ui::NotifyKind::Info);
            }
            "event-logger.answer" => {
                if let Some(card) = reviewer::current_card() {
                    record("reviewerCard", json!(card.id));
                }
                // Drive the reviewer: reveal, then pass. The app applies these.
                reviewer::show_answer();
                reviewer::answer(3);
            }
            _ => {}
        }
    }

    fn on_timer(&mut self, timer_id: String) {
        record("timerFired", json!(timer_id));
    }

    fn on_collection_loaded(&mut self) {
        record("collectionLoaded", json!(true));
    }

    fn on_card_shown(&mut self, card: Card) {
        record("lastShown", json!(card.id));
    }

    fn on_answer_shown(&mut self, card: Card) {
        record("lastAnswerShown", json!(card.id));
    }

    fn on_review_answered(&mut self, card: Card, button: u8) {
        record("lastAnswered", json!(format!("{}:{}", card.id, button)));
    }

    fn on_note_added(&mut self, note_id: u64) {
        record("lastAdded", json!(note_id));
    }

    fn on_notes_will_be_deleted(&mut self, note_ids: Vec<u64>) {
        let ids: Vec<String> = note_ids.iter().map(u64::to_string).collect();
        record("willDelete", json!(ids.join(",")));
    }

    fn transform_card_html(
        &mut self,
        _card: Card,
        _answer_side: bool,
        html: String,
    ) -> Option<String> {
        Some(format!("{html}<!-- event-logger -->"))
    }

    fn field_filter(
        &mut self,
        filter_name: String,
        _field_name: String,
        field_text: String,
        _card: Card,
    ) -> Option<String> {
        // A custom `{{shout:Field}}` filter: uppercase the field.
        (filter_name == "shout").then(|| field_text.to_uppercase())
    }
}

manatan_flashcards_sdk::export_extension!(EventLogger);
