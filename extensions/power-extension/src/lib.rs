//! Exercises the `ui` and `reviewer` capabilities: registers a Tools-menu
//! command on load, shows a notification when it's invoked, and auto-buries a
//! card answered "Easy". Demonstrates that the full capability surface works.

use manatan_flashcards_sdk::{reviewer, ui, Card, Extension};

#[derive(Default)]
struct PowerExtension;

impl Extension for PowerExtension {
    fn init(&mut self) {
        ui::register_command(&ui::Command {
            id: "power.hello".into(),
            label: "Power: Say hello".into(),
            location: ui::CommandLocation::ToolsMenu,
            shortcut: Some("Ctrl+Shift+P".into()),
        });
    }

    fn on_command(&mut self, command_id: String) {
        if command_id == "power.hello" {
            ui::notify("Hello from a WASM extension", ui::NotifyKind::Info);
        }
    }

    fn on_review_answered(&mut self, _card: Card, button: u8) {
        // 4 == "Easy": bury the card so it doesn't come back today.
        if button == 4 {
            reviewer::bury_card();
        }
    }
}

manatan_flashcards_sdk::export_extension!(PowerExtension);
