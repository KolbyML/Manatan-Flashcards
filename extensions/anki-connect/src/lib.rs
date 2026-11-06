//! An AnkiConnect-compatible API server, as a sandboxed WASM extension.
//! External tools (Yomitan, sentence miners, PDF readers) speak AnkiConnect's
//! documented JSON-over-HTTP protocol to 127.0.0.1:8765; this extension
//! implements that protocol against the generic `collection` capability, so
//! those tools work unchanged — with no AnkiConnect-specific code in the host.
//!
//! This is an original implementation written against the protocol
//! documentation and observed wire behavior; it shares no code with the
//! AnkiConnect add-on. Notes in Manatan SRS are one-note-per-card, so note ids
//! and card ids are the same value.
//!
//! The host binds the listener (`net::serve`) and delivers each request to
//! `handle_http`; disabling the extension makes the host refuse requests
//! before they reach this code.

use base64::Engine as _;
use manatan_flashcards_sdk::{collection, net, store, Extension, HttpRequest, HttpResponse};
use serde_json::{json, Value};

const API_VERSION: i64 = 6;
const DEFAULT_PORT: u16 = 8765;

const SUPPORTED_ACTIONS: &[&str] = &[
    "version",
    "apiReflect",
    "requestPermission",
    "deckNames",
    "deckNamesAndIds",
    "createDeck",
    "modelNames",
    "modelNamesAndIds",
    "modelFieldNames",
    "findNotes",
    "findCards",
    "notesInfo",
    "cardsInfo",
    "addNote",
    "addNotes",
    "canAddNotes",
    "canAddNotesWithErrorDetail",
    "updateNoteFields",
    "deleteNotes",
    "storeMediaFile",
    "retrieveMediaFile",
    "deleteMediaFile",
    "guiBrowse",
    "sync",
    "multi",
];

#[derive(Default)]
struct AnkiConnect;

impl Extension for AnkiConnect {
    fn init(&mut self) {
        // Honor the classic webBindPort config knob when the manifest grants
        // that port; otherwise fall back to AnkiConnect's canonical 8765.
        let configured = config_value()
            .get("webBindPort")
            .and_then(Value::as_u64)
            .map(|port| port as u16)
            .unwrap_or(DEFAULT_PORT);
        if net::serve(configured).is_err() && configured != DEFAULT_PORT {
            let _ = net::serve(DEFAULT_PORT);
        }
    }

    fn handle_http(&mut self, request: HttpRequest) -> HttpResponse {
        handle(&request)
    }
}

manatan_flashcards_sdk::export_extension!(AnkiConnect);

fn config_value() -> Value {
    serde_json::from_str(&store::config()).unwrap_or_else(|_| json!({}))
}

/// `webCorsOriginList` (default `["http://localhost"]`) plus the deprecated
/// single-origin `webCorsOrigin`, exactly like the reference add-on.
fn cors_origin_list(config: &Value) -> Vec<String> {
    let mut list: Vec<String> = config
        .get("webCorsOriginList")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_else(|| vec!["http://localhost".to_string()]);
    if let Some(single) = config.get("webCorsOrigin").and_then(Value::as_str) {
        list.push(single.to_string());
    }
    list
}

struct OriginDecision {
    allowed: bool,
    cors_origin: String,
}

/// AnkiConnect's origin policy: exact allowlist match; `*` allows everyone;
/// allowing `http://localhost` also admits 127.0.0.1 and browser-extension
/// origins (how Yomitan works out of the box); no Origin header (non-browser
/// clients) is always allowed.
fn allow_origin(origin: Option<&str>, config: &Value) -> OriginDecision {
    let list = cors_origin_list(config);
    if list.iter().any(|entry| entry == "*") {
        return OriginDecision {
            allowed: true,
            cors_origin: "*".to_string(),
        };
    }
    let localhost_allowed = list.iter().any(|entry| entry == "http://localhost");
    match origin {
        Some(origin) if list.iter().any(|entry| entry == origin) => OriginDecision {
            allowed: true,
            cors_origin: origin.to_string(),
        },
        Some(origin)
            if localhost_allowed
                && (origin == "http://127.0.0.1"
                    || origin == "https://127.0.0.1"
                    || origin.starts_with("http://127.0.0.1:")
                    || origin.starts_with("chrome-extension://")
                    || origin.starts_with("moz-extension://")
                    || origin.starts_with("safari-web-extension://")) =>
        {
            OriginDecision {
                allowed: true,
                cors_origin: origin.to_string(),
            }
        }
        Some(_) => OriginDecision {
            allowed: false,
            cors_origin: "http://localhost".to_string(),
        },
        None => OriginDecision {
            allowed: true,
            cors_origin: "http://localhost".to_string(),
        },
    }
}

fn header<'r>(request: &'r HttpRequest, name: &str) -> Option<&'r str> {
    request
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn json_response(status: u16, cors_origin: &str, payload: Value) -> HttpResponse {
    HttpResponse {
        status,
        headers: vec![
            ("Content-Type".into(), "application/json".into()),
            ("Access-Control-Allow-Origin".into(), cors_origin.to_string()),
            ("Access-Control-Allow-Headers".into(), "*".into()),
        ],
        body: payload.to_string().into_bytes(),
    }
}

/// The reference's 403 for requests from origins the user has not allowed.
fn forbidden(cors_origin: &str) -> HttpResponse {
    HttpResponse {
        status: 403,
        headers: vec![
            ("Access-Control-Allow-Origin".into(), cors_origin.to_string()),
            ("Access-Control-Allow-Headers".into(), "*".into()),
        ],
        body: Vec::new(),
    }
}

fn handle(request: &HttpRequest) -> HttpResponse {
    let config = config_value();
    let origin = header(request, "origin").map(str::to_string);
    let decision = allow_origin(origin.as_deref(), &config);

    if request.method.eq_ignore_ascii_case("options") {
        let mut headers = vec![
            ("Content-Type".into(), "application/json".into()),
            (
                "Access-Control-Allow-Origin".into(),
                decision.cors_origin.clone(),
            ),
            ("Access-Control-Allow-Headers".into(), "*".into()),
        ];
        // Chrome requires this for requests from https pages to localhost
        // (Yomitan's AnkiConnect access does exactly that); echoed only when
        // the preflight asks, like the reference.
        if header(request, "access-control-request-private-network") == Some("true") {
            headers.push(("Access-Control-Allow-Private-Network".into(), "true".into()));
        }
        return HttpResponse {
            status: 200,
            headers,
            body: Vec::new(),
        };
    }

    // An empty body (a plain GET, a probe) answers with the API banner.
    if request.body.is_empty() {
        if decision.allowed {
            return json_response(
                200,
                &decision.cors_origin,
                json!({"apiVersion": format!("Manatan SRS AnkiConnect v.{API_VERSION}")}),
            );
        }
        return forbidden(&decision.cors_origin);
    }

    let parsed = match serde_json::from_slice::<Value>(&request.body) {
        Ok(value) => value,
        Err(error) if decision.allowed => {
            return json_response(
                200,
                &decision.cors_origin,
                json!({"result": null, "error": format!("invalid request: {error}")}),
            );
        }
        Err(_) => return forbidden(&decision.cors_origin),
    };

    // Disallowed origins may still call requestPermission (and read its
    // response), so they can ask the user for access; everything else is 403.
    let action = parsed.get("action").and_then(Value::as_str).unwrap_or("");
    if !decision.allowed && action != "requestPermission" {
        return forbidden(&decision.cors_origin);
    }
    let cors_origin = if decision.allowed {
        decision.cors_origin.clone()
    } else {
        origin.clone().unwrap_or_default()
    };
    let reply = run_request(&parsed, origin.as_deref(), decision.allowed);
    json_response(200, &cors_origin, reply)
}

/// One request through the reference `handler`: api-key gate, dispatch, and
/// version-shaped reply. `multi` routes each sub-request back through here, so
/// keys and versions apply per sub-request exactly like the add-on.
fn run_request(request: &Value, origin: Option<&str>, allowed: bool) -> Value {
    let action = request.get("action").and_then(Value::as_str).unwrap_or("");
    // The reference defaults an absent version to 4.
    let version = request.get("version").and_then(Value::as_i64).unwrap_or(4);
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    // `key` must equal the configured `apiKey` — both default to null, and
    // requestPermission is exempt so clients can discover the requirement.
    let key = request.get("key").cloned().unwrap_or(Value::Null);
    let api_key = config_value().get("apiKey").cloned().unwrap_or(Value::Null);
    let outcome = if key != api_key && action != "requestPermission" {
        Err("valid api key must be provided".to_string())
    } else {
        run_action(action, &params, origin, allowed)
    };
    match outcome {
        // Protocol versions 4 and below predate the result envelope.
        Ok(value) if version <= 4 => value,
        Ok(value) => json!({"result": value, "error": null}),
        Err(error) => json!({"result": null, "error": error}),
    }
}

fn run_action(
    action: &str,
    params: &Value,
    origin: Option<&str>,
    allowed: bool,
) -> Result<Value, String> {
    match action {
        "version" => Ok(json!(API_VERSION)),
        "apiReflect" => Ok(json!({
            "scopes": ["actions"],
            "actions": SUPPORTED_ACTIONS,
        })),
        "requestPermission" => Ok(request_permission(origin, allowed)),
        "multi" => {
            let mut results = Vec::new();
            if let Some(actions) = params.get("actions").and_then(Value::as_array) {
                for sub_request in actions {
                    results.push(run_request(sub_request, origin, allowed));
                }
            }
            Ok(Value::Array(results))
        }
        "deckNames" => Ok(json!(
            deck_names_and_ids()
                .into_iter()
                .map(|(name, _)| name)
                .collect::<Vec<_>>()
        )),
        "deckNamesAndIds" => {
            let mut map = serde_json::Map::new();
            for (name, id) in deck_names_and_ids() {
                map.insert(name, json!(id));
            }
            Ok(Value::Object(map))
        }
        "createDeck" => create_deck(params),
        "modelNames" => Ok(json!(
            collection::list_notetypes()
                .into_iter()
                .map(|(_, name)| name)
                .collect::<Vec<_>>()
        )),
        "modelNamesAndIds" => {
            let mut map = serde_json::Map::new();
            for (id, name) in collection::list_notetypes() {
                map.insert(name, json!(id));
            }
            Ok(Value::Object(map))
        }
        "modelFieldNames" => {
            let wanted = params
                .get("modelName")
                .and_then(Value::as_str)
                .ok_or("modelName is required")?;
            let (id, _) = resolve_model(wanted)?;
            let notetype =
                collection::get_notetype(id).ok_or(format!("model was not found: {wanted}"))?;
            Ok(json!(notetype.fields))
        }
        "findNotes" | "findCards" | "guiBrowse" => {
            let query = params.get("query").and_then(Value::as_str).unwrap_or("");
            Ok(json!(collection::find_notes(query)?))
        }
        "notesInfo" => notes_info(params),
        "cardsInfo" => cards_info(params),
        "addNote" => {
            let note = params.get("note").ok_or("note is required")?;
            add_note(note).map(|id| json!(id))
        }
        "addNotes" => {
            let notes = params
                .get("notes")
                .and_then(Value::as_array)
                .ok_or("notes are required")?;
            let mut ids = Vec::new();
            for note in notes {
                ids.push(match add_note(note) {
                    Ok(id) => json!(id),
                    Err(_) => Value::Null,
                });
            }
            Ok(Value::Array(ids))
        }
        "canAddNotes" => {
            let notes = params
                .get("notes")
                .and_then(Value::as_array)
                .ok_or("notes are required")?;
            let mut results = Vec::new();
            for note in notes {
                results.push(json!(can_add_note(note).is_ok()));
            }
            Ok(Value::Array(results))
        }
        "canAddNotesWithErrorDetail" => {
            let notes = params
                .get("notes")
                .and_then(Value::as_array)
                .ok_or("notes are required")?;
            let mut results = Vec::new();
            for note in notes {
                results.push(match can_add_note(note) {
                    Ok(()) => json!({"canAdd": true}),
                    Err(error) => json!({"canAdd": false, "error": error}),
                });
            }
            Ok(Value::Array(results))
        }
        "updateNoteFields" => update_note_fields(params),
        "deleteNotes" => {
            let ids = id_list(params, "notes")?;
            collection::delete_notes(&ids)?;
            Ok(Value::Null)
        }
        "storeMediaFile" => store_media_item(params).map(|filename| json!(filename)),
        "retrieveMediaFile" => {
            let filename = safe_media_filename(
                params
                    .get("filename")
                    .and_then(Value::as_str)
                    .ok_or("filename is required")?,
            )?;
            Ok(match collection::retrieve_media(&filename) {
                Some(bytes) => json!(base64::engine::general_purpose::STANDARD.encode(bytes)),
                None => json!(false),
            })
        }
        "deleteMediaFile" => {
            let filename = safe_media_filename(
                params
                    .get("filename")
                    .and_then(Value::as_str)
                    .ok_or("filename is required")?,
            )?;
            let _ = collection::trash_media(&[filename]);
            Ok(Value::Null)
        }
        "sync" => Ok(Value::Null),
        _ => Err("unsupported action".to_string()),
    }
}

/// AnkiConnect asks the user for permission with a dialog; the backend has no
/// dialog, so unknown origins are granted and recorded in the extension
/// config, where they can be reviewed or removed.
fn request_permission(origin: Option<&str>, allowed: bool) -> Value {
    let config = config_value();
    // The reference's `requireApikey` (sic) reports whether an apiKey is set.
    let require_api_key = !config
        .get("apiKey")
        .cloned()
        .unwrap_or(Value::Null)
        .is_null();
    let granted = json!({
        "permission": "granted",
        "requireApikey": require_api_key,
        "version": API_VERSION,
    });
    if allowed {
        return granted;
    }
    let origin = origin.unwrap_or_default();
    // Origins the user chose to ignore stay denied, silently.
    let ignored = config
        .get("ignoreOriginList")
        .and_then(Value::as_array)
        .map(|items| items.iter().any(|item| item.as_str() == Some(origin)))
        .unwrap_or(false);
    if ignored {
        return json!({"permission": "denied"});
    }
    // The reference prompts the user with a dialog here; the backend has no
    // dialog, so unknown origins are granted and recorded in the config,
    // where they can be reviewed, removed, or moved to ignoreOriginList.
    let mut config = config;
    if let Some(object) = config.as_object_mut() {
        let list = object
            .entry("webCorsOriginList")
            .or_insert_with(|| json!(["http://localhost"]));
        if let Some(items) = list.as_array_mut() {
            items.push(json!(origin));
        }
        store::set_config(&config.to_string());
    }
    granted
}

/// Deck names in Anki's `::` form, from the collection's `/`-separated paths.
fn deck_names_and_ids() -> Vec<(String, u64)> {
    collection::list_decks()
        .into_iter()
        .map(|deck| (deck.name.replace('/', "::"), deck.id))
        .collect()
}

fn resolve_deck(deck_name: &str) -> Result<(String, u64), String> {
    let wanted = deck_name.replace("::", "/");
    collection::list_decks()
        .into_iter()
        .find(|deck| deck.name == wanted)
        .map(|deck| (deck.name, deck.id))
        .ok_or(format!("deck was not found: {deck_name}"))
}

fn resolve_model(model_name: &str) -> Result<(u64, String), String> {
    collection::list_notetypes()
        .into_iter()
        .find(|(_, name)| name == model_name)
        .ok_or(format!("model was not found: {model_name}"))
}

fn create_deck(params: &Value) -> Result<Value, String> {
    let name = params
        .get("deck")
        .and_then(Value::as_str)
        .ok_or("deck is required")?;
    if let Ok((_, existing)) = resolve_deck(name) {
        return Ok(json!(existing));
    }
    let id = collection::add_deck(&name.replace("::", "/"))?;
    Ok(json!(id))
}

fn id_list(params: &Value, key: &str) -> Result<Vec<u64>, String> {
    Ok(params
        .get(key)
        .and_then(Value::as_array)
        .ok_or(format!("{key} are required"))?
        .iter()
        .filter_map(Value::as_u64)
        .collect())
}

fn note_fields_json(note: &manatan_flashcards_sdk::Note) -> Value {
    let mut fields = serde_json::Map::new();
    for (order, field) in note.fields.iter().enumerate() {
        fields.insert(
            field.name.clone(),
            json!({"value": field.value, "order": order}),
        );
    }
    Value::Object(fields)
}

fn notes_info(params: &Value) -> Result<Value, String> {
    let ids = id_list(params, "notes")?;
    let mut results = Vec::new();
    for id in ids {
        let Some(note) = collection::get_note(id) else {
            continue;
        };
        results.push(json!({
            "noteId": note.id,
            "modelName": note.notetype_name,
            "tags": note.tags,
            "fields": note_fields_json(&note),
            "cards": [note.id],
        }));
    }
    Ok(Value::Array(results))
}

fn cards_info(params: &Value) -> Result<Value, String> {
    let ids = id_list(params, "cards")?;
    let decks: Vec<(u64, String)> = collection::list_decks()
        .into_iter()
        .map(|deck| (deck.id, deck.name.replace('/', "::")))
        .collect();
    let deck_name = |id: u64| {
        decks
            .iter()
            .find(|(deck_id, _)| *deck_id == id)
            .map(|(_, name)| name.clone())
            .unwrap_or_default()
    };
    let mut results = Vec::new();
    for id in ids {
        let Some(note) = collection::get_note(id) else {
            continue;
        };
        results.push(json!({
            "cardId": note.id,
            "note": note.id,
            "deckName": deck_name(note.deck_id),
            "modelName": note.notetype_name,
            "fields": note_fields_json(&note),
        }));
    }
    Ok(Value::Array(results))
}

fn note_request_parts(note: &Value) -> Result<(String, String, Vec<(String, String)>), String> {
    let deck_name = note
        .get("deckName")
        .and_then(Value::as_str)
        .ok_or("deckName is required")?;
    let model_name = note
        .get("modelName")
        .and_then(Value::as_str)
        .ok_or("modelName is required")?;
    let fields = note
        .get("fields")
        .and_then(Value::as_object)
        .ok_or("fields are required")?
        .iter()
        .map(|(name, value)| (name.clone(), value.as_str().unwrap_or_default().to_string()))
        .collect();
    Ok((deck_name.to_string(), model_name.to_string(), fields))
}

/// Duplicate detection like AnkiConnect's: same model, same first field.
/// The first-field value is matched exactly through the host's `Field:value`
/// search; values containing double quotes escape that syntax and are treated
/// as unique.
fn first_field_duplicate(model_name: &str, first_field: &str, first_value: &str) -> bool {
    if first_value.is_empty() || first_value.contains('"') {
        return false;
    }
    let query = format!("\"{first_field}:{first_value}\"");
    let Ok(ids) = collection::find_notes(&query) else {
        return false;
    };
    ids.into_iter().any(|id| {
        collection::get_note(id)
            .map(|note| note.notetype_name == model_name)
            .unwrap_or(false)
    })
}

fn can_add_note(note: &Value) -> Result<(), String> {
    let (deck_name, model_name, fields) = note_request_parts(note)?;
    resolve_deck(&deck_name)?;
    let (model_id, model_name) = resolve_model(&model_name)?;
    let notetype =
        collection::get_notetype(model_id).ok_or(format!("model was not found: {model_name}"))?;
    let first_field = notetype.fields.first().ok_or("model has no fields")?;
    let first_value = fields
        .iter()
        .find(|(name, _)| name == first_field)
        .map(|(_, value)| value.clone())
        .unwrap_or_default();
    if first_value.is_empty() {
        return Err("cannot create note because it is empty".to_string());
    }
    let allow_duplicate = note
        .pointer("/options/allowDuplicate")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !allow_duplicate && first_field_duplicate(&model_name, first_field, &first_value) {
        return Err("cannot create note because it is a duplicate".to_string());
    }
    Ok(())
}

fn add_note(note: &Value) -> Result<u64, String> {
    can_add_note(note)?;
    let (deck_name, model_name, fields) = note_request_parts(note)?;
    let deck = deck_name.replace("::", "/");
    let mut fields: Vec<(String, String)> = fields;

    // Media attachments ([sound:...] tags and <img> references), like
    // AnkiConnect's audio/picture/video note members.
    for (kind, key) in [
        ("audio", "audio"),
        ("picture", "picture"),
        ("video", "video"),
    ] {
        if let Some(items) = note.get(key).and_then(Value::as_array) {
            for item in items {
                let Ok(filename) = store_media_item(item) else {
                    continue;
                };
                let targets = item
                    .get("fields")
                    .and_then(Value::as_array)
                    .map(|list| {
                        list.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                for target in targets {
                    let suffix = if kind == "audio" {
                        format!("[sound:{filename}]")
                    } else {
                        format!("<img src=\"{filename}\">")
                    };
                    match fields.iter_mut().find(|(name, _)| *name == target) {
                        Some((_, value)) => value.push_str(&suffix),
                        None => fields.push((target, suffix)),
                    }
                }
            }
        }
    }

    let tags: Vec<String> = note
        .get("tags")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    collection::add_note(&deck, &model_name, &fields, &tags)
}

fn update_note_fields(params: &Value) -> Result<Value, String> {
    let note = params.get("note").ok_or("note is required")?;
    let id = note
        .get("id")
        .and_then(Value::as_u64)
        .ok_or("note id is required")?;
    let fields: Vec<(String, String)> = note
        .get("fields")
        .and_then(Value::as_object)
        .ok_or("fields are required")?
        .iter()
        .map(|(name, value)| (name.clone(), value.as_str().unwrap_or_default().to_string()))
        .collect();
    collection::update_note_fields(id, &fields)?;
    Ok(Value::Null)
}

fn safe_media_filename(filename: &str) -> Result<String, String> {
    if filename.is_empty()
        || filename.contains('/')
        || filename.contains('\\')
        || filename.starts_with('.')
    {
        return Err(format!("invalid filename: {filename}"));
    }
    Ok(filename.to_string())
}

/// Store one media item ({filename, data|url}) and return its filename.
fn store_media_item(item: &Value) -> Result<String, String> {
    let filename = safe_media_filename(
        item.get("filename")
            .and_then(Value::as_str)
            .ok_or("filename is required")?,
    )?;
    let bytes = if let Some(data) = item.get("data").and_then(Value::as_str) {
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| e.to_string())?
    } else if let Some(url) = item.get("url").and_then(Value::as_str) {
        let response = net::fetch(&HttpRequest {
            method: "GET".into(),
            url: url.to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        })?;
        if response.status >= 400 {
            return Err(format!("download failed with status {}", response.status));
        }
        response.body
    } else {
        return Err("data or url is required".to_string());
    };
    collection::store_media(&filename, &bytes)
}
