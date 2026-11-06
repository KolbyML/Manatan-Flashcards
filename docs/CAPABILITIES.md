# Manatan Flashcards extension capabilities

Extensions use host-mediated interfaces defined in
[`world.wit`](../crates/manatan-flashcards-sdk/wit/world.wit). Request only the
permissions your feature needs. A declared interface is an API contract, not
a guarantee that every host platform implements every underlying action.

## Study tools and collection data

| Feature | Interface |
|---|---|
| Review events and custom answer labels | `Extension` review hooks |
| Review actions and editing the current note | `reviewer` |
| Interval overrides and deck scheduling options | `override_scheduling`, `scheduler` |
| Search, note fields, tags, note types, and media storage | `collection` |
| Review history and upcoming due counts | `collection::review_log`, `collection::due_forecast` |
| Custom template filters and rendered card transforms | `field_filter`, `transform_card_html` |

## Interface extensions

Render HTML fragments into dashboard, overview, reviewer, browser, editor,
stats, and deck-options surfaces. Use `ui` for commands, shortcuts, browser
columns, editor buttons, dialogs, and notifications. Use `store` for
per-extension configuration and persisted state.

The examples demonstrate a dashboard panel, a review counter, a calendar
heatmap, lifecycle logging, and a command that buries the current card.

## Integrations

Use `net::fetch` for permitted remote URL prefixes. Use `net::serve` and
`handle_http` for a host-managed local HTTP integration on a declared port.
The compatibility bridge example retains its existing protocol so tools
that already speak it do not need to change.

`files` provides user-mediated open and save dialogs. `media` exposes
host-supported playback, speech, screenshots, and clipboard operations.
`tasks` provides timers; `host` provides logging, time, theme, and locale.

## Sandbox boundaries

Extensions have no ambient filesystem or socket access and cannot spawn
native processes. File access goes through a picker, network access goes
through the host, and UI changes use surfaces and registered controls rather
than arbitrary native widget manipulation. Permission enforcement is the
host's responsibility; test denied access as well as the successful path.
