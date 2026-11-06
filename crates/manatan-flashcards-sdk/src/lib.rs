//! # Manatan Flashcards extension SDK
//!
//! Build [Manatan Flashcards](https://manatan.com/srs) extensions as sandboxed
//! WebAssembly components in Rust. This crate is fully public and has **no
//! dependency on Manatan's closed-source code** — only [`wit_bindgen`] and the
//! WIT contract in `wit/world.wit`.
//!
//! Extend study workflows through capability-gated interfaces mediated by
//! the host, without ambient filesystem, network, or native process access.
//!
//! An extension:
//! 1. implements [`Extension`] (every hook has a default — write only what you
//!    need), and
//! 2. registers it with [`export_extension!`].
//!
//! It calls host capabilities — [`collection`], [`scheduler`], [`reviewer`],
//! [`ui`], [`files`], [`media`], [`net`], [`store`], [`tasks`], [`host`] — but
//! only those its manifest requests and the user grants.
//!
//! ```ignore
//! use manatan_flashcards_sdk::{collection, Extension};
//!
//! #[derive(Default)]
//! struct DeckCounter;
//! impl Extension for DeckCounter {
//!     fn render(&mut self, surface: String, _ctx: String) -> String {
//!         if surface != "dashboard" { return String::new(); }
//!         format!("<p>{} decks</p>", collection::list_decks().len())
//!     }
//! }
//! manatan_flashcards_sdk::export_extension!(DeckCounter);
//! ```

wit_bindgen::generate!({
    world: "extension",
    path: "wit",
    pub_export_macro: true,
    export_macro_name: "export_extension_raw",
});

// Friendly re-exports so extensions write `manatan_flashcards_sdk::collection::…`.
pub use crate::manatan::srs::{
    collection, files, host, media, net, reviewer, scheduler, store, tasks, types, ui,
};

// Hoist the most-used data types to the crate root.
pub use net::{HttpRequest, HttpResponse};
pub use types::{
    Card, CardFlag, CardQueue, Deck, Field, Note, Notetype, Rating, ReviewLogEntry,
    SchedulingState, Template,
};

// `export_extension_raw` is generated at the crate root by `generate!`
// (pub_export_macro), so `$crate::export_extension_raw!` already resolves.
#[doc(hidden)]
pub use crate::exports::manatan::srs::guest::Guest as __Guest;

use core::any::Any;
use core::cell::RefCell;

/// A Manatan Flashcards extension.
///
/// Implement only the hooks you need — all have no-op defaults — then register
/// your type with [`export_extension!`]. Instance state lives in `self`; use
/// [`store`] for state that must survive restarts.
///
/// Hooks cover lifecycle, the review session,
/// note/card data, rendering + transforms, the editor, the browser, UI
/// commands, and app events.
pub trait Extension: Default + 'static {
    // --- lifecycle ---
    fn init(&mut self) {}
    fn on_collection_loaded(&mut self) {}
    fn on_collection_closing(&mut self) {}
    fn on_profile_open(&mut self) {}
    fn on_day_changed(&mut self) {}

    // --- review session ---
    fn on_card_shown(&mut self, card: Card) {
        let _ = card;
    }
    fn on_answer_shown(&mut self, card: Card) {
        let _ = card;
    }
    fn on_review_answered(&mut self, card: Card, button: u8) {
        let _ = (card, button);
    }
    fn on_will_bury(&mut self, card: Card) {
        let _ = card;
    }
    fn on_will_suspend(&mut self, card: Card) {
        let _ = card;
    }
    fn on_review_ended(&mut self) {}
    /// Customize the answer-button labels; return `defaults` to keep them.
    fn answer_button_labels(&mut self, card: Card, defaults: Vec<String>) -> Vec<String> {
        let _ = card;
        defaults
    }
    /// Override the scheduler's post-answer interval (custom scheduling / load
    /// balancers). Return `None` to keep the default.
    fn override_scheduling(
        &mut self,
        card: Card,
        rating: Rating,
        default: SchedulingState,
    ) -> Option<SchedulingState> {
        let _ = (card, rating, default);
        None
    }

    // --- note / card data ---
    /// Transform a note just before it is added. Return `None` to leave it.
    fn on_note_will_be_added(&mut self, note: Note) -> Option<Note> {
        let _ = note;
        None
    }
    fn on_note_added(&mut self, note_id: u64) {
        let _ = note_id;
    }
    fn on_notes_will_be_deleted(&mut self, note_ids: Vec<u64>) {
        let _ = note_ids;
    }

    // --- rendering / transforms ---
    /// Contribute an HTML fragment to a surface (`"dashboard"`, `"reviewer"`,
    /// …). Return `""` to render nothing.
    fn render(&mut self, surface: String, context: String) -> String {
        let _ = (surface, context);
        String::new()
    }
    /// Transform a rendered card's HTML. Return `None` to keep it.
    fn transform_card_html(
        &mut self,
        card: Card,
        answer_side: bool,
        html: String,
    ) -> Option<String> {
        let _ = (card, answer_side, html);
        None
    }
    /// Implement a custom template field filter (`{{name:field}}`). Return
    /// `None` if this extension doesn't handle `filter_name`.
    fn field_filter(
        &mut self,
        filter_name: String,
        field_name: String,
        field_text: String,
        card: Card,
    ) -> Option<String> {
        let _ = (filter_name, field_name, field_text, card);
        None
    }

    // --- editor ---
    fn on_editor_loaded(&mut self, note_id: u64) {
        let _ = note_id;
    }
    fn on_editor_field_changed(&mut self, note_id: u64, field_ord: u32, value: String) {
        let _ = (note_id, field_ord, value);
    }
    /// Transform pasted HTML in the editor. Return `None` to keep it.
    fn on_editor_paste(&mut self, html: String) -> Option<String> {
        let _ = html;
        None
    }
    fn on_editor_button(&mut self, button_id: String, note_id: u64) {
        let _ = (button_id, note_id);
    }

    // --- browser ---
    /// Modify the search before it runs. Return `None` to keep it.
    fn on_browser_search(&mut self, search: String) -> Option<String> {
        let _ = search;
        None
    }
    /// Value for a registered custom column, per card.
    fn browser_column_value(&mut self, column_id: String, card: Card) -> String {
        let _ = (column_id, card);
        String::new()
    }

    // --- ui / messages ---
    fn on_command(&mut self, command_id: String) {
        let _ = command_id;
    }
    /// A message from injected surface JS or an `ui::open-dialog` bridge.
    /// Return a reply string.
    fn on_message(&mut self, source: String, payload: String) -> String {
        let _ = (source, payload);
        String::new()
    }
    fn on_timer(&mut self, timer_id: String) {
        let _ = timer_id;
    }

    // --- app events ---
    fn on_state_changed(&mut self, old_state: String, new_state: String) {
        let _ = (old_state, new_state);
    }
    fn on_sync_started(&mut self) {}
    fn on_sync_finished(&mut self) {}

    // --- http serve ---
    /// Handle a request on a port registered via [`net::serve`].
    fn handle_http(&mut self, request: HttpRequest) -> HttpResponse {
        let _ = request;
        HttpResponse {
            status: 404,
            headers: Vec::new(),
            body: b"not found".to_vec(),
        }
    }
}

/// Dispatch into the single [`Extension`] instance for this component. WASM
/// guests are single-threaded, so one thread-local singleton suffices; it is
/// created lazily on first use.
#[doc(hidden)]
pub fn __dispatch<T: Extension, R>(f: impl FnOnce(&mut T) -> R) -> R {
    thread_local! {
        static STATE: RefCell<Option<Box<dyn Any>>> = const { RefCell::new(None) };
    }
    STATE.with(|cell| {
        let mut slot = cell.borrow_mut();
        let boxed = slot.get_or_insert_with(|| Box::new(T::default()) as Box<dyn Any>);
        let ext = boxed
            .downcast_mut::<T>()
            .expect("a component registers exactly one extension type");
        f(ext)
    })
}

/// Register your [`Extension`] type as the component's entry point. Call once,
/// at the crate root, with a type implementing [`Extension`].
#[macro_export]
macro_rules! export_extension {
    ($ext:ty) => {
        #[doc(hidden)]
        struct __ManatanExtensionComponent;

        impl $crate::__Guest for __ManatanExtensionComponent {
            fn init() {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::init(e));
            }
            fn on_collection_loaded() {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_collection_loaded(e));
            }
            fn on_collection_closing() {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_collection_closing(e));
            }
            fn on_profile_open() {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_profile_open(e));
            }
            fn on_day_changed() {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_day_changed(e));
            }
            fn on_card_shown(card: $crate::Card) {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_card_shown(e, card));
            }
            fn on_answer_shown(card: $crate::Card) {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_answer_shown(e, card));
            }
            fn on_review_answered(card: $crate::Card, button: u8) {
                $crate::__dispatch::<$ext, _>(|e| {
                    $crate::Extension::on_review_answered(e, card, button)
                });
            }
            fn on_will_bury(card: $crate::Card) {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_will_bury(e, card));
            }
            fn on_will_suspend(card: $crate::Card) {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_will_suspend(e, card));
            }
            fn on_review_ended() {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_review_ended(e));
            }
            fn answer_button_labels(
                card: $crate::Card,
                defaults: ::std::vec::Vec<::std::string::String>,
            ) -> ::std::vec::Vec<::std::string::String> {
                $crate::__dispatch::<$ext, _>(|e| {
                    $crate::Extension::answer_button_labels(e, card, defaults)
                })
            }
            fn override_scheduling(
                card: $crate::Card,
                rating: $crate::Rating,
                default: $crate::SchedulingState,
            ) -> ::core::option::Option<$crate::SchedulingState> {
                $crate::__dispatch::<$ext, _>(|e| {
                    $crate::Extension::override_scheduling(e, card, rating, default)
                })
            }
            fn on_note_will_be_added(
                note: $crate::Note,
            ) -> ::core::option::Option<$crate::Note> {
                $crate::__dispatch::<$ext, _>(|e| {
                    $crate::Extension::on_note_will_be_added(e, note)
                })
            }
            fn on_note_added(note_id: u64) {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_note_added(e, note_id));
            }
            fn on_notes_will_be_deleted(note_ids: ::std::vec::Vec<u64>) {
                $crate::__dispatch::<$ext, _>(|e| {
                    $crate::Extension::on_notes_will_be_deleted(e, note_ids)
                });
            }
            fn render(
                surface: ::std::string::String,
                context: ::std::string::String,
            ) -> ::std::string::String {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::render(e, surface, context))
            }
            fn transform_card_html(
                card: $crate::Card,
                answer_side: bool,
                html: ::std::string::String,
            ) -> ::core::option::Option<::std::string::String> {
                $crate::__dispatch::<$ext, _>(|e| {
                    $crate::Extension::transform_card_html(e, card, answer_side, html)
                })
            }
            fn field_filter(
                filter_name: ::std::string::String,
                field_name: ::std::string::String,
                field_text: ::std::string::String,
                card: $crate::Card,
            ) -> ::core::option::Option<::std::string::String> {
                $crate::__dispatch::<$ext, _>(|e| {
                    $crate::Extension::field_filter(e, filter_name, field_name, field_text, card)
                })
            }
            fn on_editor_loaded(note_id: u64) {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_editor_loaded(e, note_id));
            }
            fn on_editor_field_changed(
                note_id: u64,
                field_ord: u32,
                value: ::std::string::String,
            ) {
                $crate::__dispatch::<$ext, _>(|e| {
                    $crate::Extension::on_editor_field_changed(e, note_id, field_ord, value)
                });
            }
            fn on_editor_paste(
                html: ::std::string::String,
            ) -> ::core::option::Option<::std::string::String> {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_editor_paste(e, html))
            }
            fn on_editor_button(button_id: ::std::string::String, note_id: u64) {
                $crate::__dispatch::<$ext, _>(|e| {
                    $crate::Extension::on_editor_button(e, button_id, note_id)
                });
            }
            fn on_browser_search(
                search: ::std::string::String,
            ) -> ::core::option::Option<::std::string::String> {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_browser_search(e, search))
            }
            fn browser_column_value(
                column_id: ::std::string::String,
                card: $crate::Card,
            ) -> ::std::string::String {
                $crate::__dispatch::<$ext, _>(|e| {
                    $crate::Extension::browser_column_value(e, column_id, card)
                })
            }
            fn on_command(command_id: ::std::string::String) {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_command(e, command_id));
            }
            fn on_message(
                source: ::std::string::String,
                payload: ::std::string::String,
            ) -> ::std::string::String {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_message(e, source, payload))
            }
            fn on_timer(timer_id: ::std::string::String) {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_timer(e, timer_id));
            }
            fn on_state_changed(
                old_state: ::std::string::String,
                new_state: ::std::string::String,
            ) {
                $crate::__dispatch::<$ext, _>(|e| {
                    $crate::Extension::on_state_changed(e, old_state, new_state)
                });
            }
            fn on_sync_started() {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_sync_started(e));
            }
            fn on_sync_finished() {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::on_sync_finished(e));
            }
            fn handle_http(request: $crate::HttpRequest) -> $crate::HttpResponse {
                $crate::__dispatch::<$ext, _>(|e| $crate::Extension::handle_http(e, request))
            }
        }

        $crate::export_extension_raw!(__ManatanExtensionComponent with_types_in $crate);
    };
}
