//! What the host does with a well-formed request, and what it refuses.

use nopass_core::crypto::generate_identity_locked;
use nopass_core::lock::SecretString;
use nopass_core::{PlainCrypto, Store};
use nopass_host::session::Session;
use nopass_host::{proto, Host};
use serde_json::{json, Value};
use tempfile::TempDir;

/// A store on disk with no encryption, so these tests exercise dispatch
/// rather than crypto. `PlainCrypto` exists for exactly this.
fn store_with(entries: &[(&str, &str)]) -> (TempDir, Store) {
    let dir = TempDir::new().expect("a temporary directory");
    let store = Store::open(dir.path(), Box::new(PlainCrypto));
    // PlainCrypto ignores recipients, but the store still refuses to write
    // into a directory that names none.
    store
        .init(&["test-recipient".to_string()], "")
        .expect("the store initialises");

    for (name, body) in entries {
        store
            .insert(name, body.as_bytes())
            .expect("the entry writes");
    }
    (dir, store)
}

fn sample() -> (TempDir, Host) {
    let (dir, store) = store_with(&[
        (
            "web/google.com",
            "hunter2\nusername: sana@example.com\nurl: https://google.com\notpauth://totp/g?secret=JBSWY3DPEHPK3PXP\n",
        ),
        ("web/github.com", "octocat-pw\nusername: sana\n"),
        ("mail/fastmail", "fm-pw\n"),
        (
            "work/intranet",
            "intranet-pw\nusername: sana@work.example\nurl: https://portal.example.org\n",
        ),
    ]);
    let session = Session::new(dir.path().join("identity.txt"));
    (dir, Host::with_session(store, session))
}

/// The same store, but behind an identity that really is locked.
///
/// `Session::default()` would read the identity of whoever is running the
/// tests, so a lock-state assertion made against it would pass or fail
/// depending on whether that person happened to have unlocked their own store.
/// This one is a throwaway locked file nothing holds the passphrase for.
fn sample_locked() -> (TempDir, Host) {
    let (dir, store) = store_with(&[("web/github.com", "octocat-pw\nusername: sana\n")]);
    let identity = dir.path().join("identity.txt");
    generate_identity_locked(&identity, &SecretString::from("correct-horse".to_owned()))
        .expect("a locked identity is written");
    (dir, Host::with_session(store, Session::new(identity)))
}

/// Every reply the host emits must itself satisfy the published schema.
fn reply(host: &mut Host, request: Value) -> Value {
    let response = host.handle(&request);
    proto::parse_response(&response)
        .unwrap_or_else(|e| panic!("the host emitted an off-contract response: {e}\n{response}"));
    response
}

/// One canonical field out of a `get` reply.
fn field<'a>(entry: &'a Value, key: &str) -> Option<&'a str> {
    entry["fields"]
        .as_array()?
        .iter()
        .find(|field| field["key"] == json!(key))?["value"]
        .as_str()
}

/// The entry as it sits on disk, read through a second handle on the same
/// store — what the host wrote is the thing under test.
fn body_of(root: &std::path::Path, name: &str) -> String {
    let store = Store::open(root, Box::new(PlainCrypto));
    String::from_utf8(store.show(name).expect("the entry reads back")).expect("utf-8")
}

fn error_code(response: &Value) -> &str {
    response["error"]["code"]
        .as_str()
        .expect("an error has a code")
}

#[test]
fn hello_reports_the_protocol_version() {
    let (_dir, mut host) = sample();
    let response = reply(&mut host, json!({ "id": 1, "verb": "hello", "version": 1 }));

    assert_eq!(response["ok"], json!(true));
    assert_eq!(response["version"], json!(proto::PROTOCOL_VERSION));
    assert_eq!(response["store"], json!("ready"));
}

#[test]
fn hello_reports_a_store_that_was_never_initialised() {
    let dir = TempDir::new().expect("a temporary directory");
    let mut host = Host::new(Store::open(
        dir.path().join("absent"),
        Box::new(PlainCrypto),
    ));

    let response = reply(&mut host, json!({ "id": 1, "verb": "hello", "version": 1 }));
    assert_eq!(response["store"], json!("missing"));
}

#[test]
fn list_returns_names_and_no_secrets() {
    let (_dir, mut host) = sample();
    let response = reply(&mut host, json!({ "id": 2, "verb": "list" }));

    let entries = response["entries"].as_array().expect("a list of entries");
    let names: Vec<&str> = entries.iter().map(|e| e.as_str().unwrap()).collect();
    assert!(names.contains(&"web/google.com"), "{names:?}");
    assert!(names.contains(&"mail/fastmail"), "{names:?}");

    let text = response.to_string();
    for secret in ["hunter2", "octocat-pw", "fm-pw"] {
        assert!(!text.contains(secret), "list leaked {secret}");
    }
}

#[test]
fn search_offers_only_entries_for_that_origin() {
    let (_dir, mut host) = sample();
    let response = reply(
        &mut host,
        json!({ "id": 3, "verb": "search", "origin": "https://mail.google.com" }),
    );

    let matches = response["matches"].as_array().expect("a list of matches");
    let names: Vec<&str> = matches
        .iter()
        .map(|m| m["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["web/google.com"]);
    assert_eq!(matches[0]["username"], json!("sana@example.com"));
    assert!(
        !response.to_string().contains("hunter2"),
        "search leaked a password"
    );
}

#[test]
fn search_offers_an_entry_matched_only_by_its_url_line() {
    let (_dir, mut host) = sample();
    let response = reply(
        &mut host,
        json!({ "id": 3, "verb": "search", "origin": "https://portal.example.org" }),
    );

    let matches = response["matches"].as_array().expect("a list of matches");
    let names: Vec<&str> = matches
        .iter()
        .map(|m| m["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["work/intranet"]);
    assert_eq!(matches[0]["url"], json!("https://portal.example.org"));
}

#[test]
fn search_on_an_unrelated_origin_offers_nothing() {
    let (_dir, mut host) = sample();
    let response = reply(
        &mut host,
        json!({ "id": 4, "verb": "search", "origin": "https://google.com.evil.tld" }),
    );

    assert_eq!(response["matches"], json!([]));
}

#[test]
fn get_returns_the_password_and_the_metadata_around_it() {
    let (_dir, mut host) = sample();
    let response = reply(
        &mut host,
        json!({ "id": 5, "verb": "get", "entry": "web/google.com" }),
    );

    let entry = &response["entry"];
    assert_eq!(entry["kind"], json!("login"));
    assert_eq!(entry["secret"], json!("hunter2"));
    assert_eq!(field(entry, "username"), Some("sana@example.com"));
    assert_eq!(field(entry, "url"), Some("https://google.com"));
    assert_eq!(
        field(entry, "totp"),
        Some("otpauth://totp/g?secret=JBSWY3DPEHPK3PXP")
    );
}

#[test]
fn get_returns_an_entry_that_is_only_a_password() {
    let (_dir, mut host) = sample();
    let response = reply(
        &mut host,
        json!({ "id": 6, "verb": "get", "entry": "mail/fastmail" }),
    );

    assert_eq!(response["entry"]["secret"], json!("fm-pw"));
    assert_eq!(response["entry"]["fields"], json!([]));
}

#[test]
fn get_on_an_absent_entry_says_so() {
    let (_dir, mut host) = sample();
    let response = reply(
        &mut host,
        json!({ "id": 7, "verb": "get", "entry": "web/nowhere" }),
    );

    assert_eq!(response["ok"], json!(false));
    assert_eq!(error_code(&response), "not_found");
}

#[test]
fn get_refuses_to_climb_out_of_the_store() {
    let (_dir, mut host) = sample();

    for entry in ["../../../etc/passwd", "/etc/passwd", "web/../../secret"] {
        let response = reply(&mut host, json!({ "id": 8, "verb": "get", "entry": entry }));
        assert_eq!(response["ok"], json!(false), "{entry} was not refused");
    }
}

#[test]
fn generate_returns_a_password_of_the_requested_length() {
    let (_dir, mut host) = sample();
    let response = reply(
        &mut host,
        json!({ "id": 9, "verb": "generate", "length": 24, "symbols": true }),
    );

    assert_eq!(
        response["password"]
            .as_str()
            .expect("a password")
            .chars()
            .count(),
        24
    );
}

#[test]
fn generate_does_not_store_what_it_produced() {
    let (_dir, mut host) = sample();
    let before = reply(&mut host, json!({ "id": 10, "verb": "list" }));
    reply(
        &mut host,
        json!({ "id": 11, "verb": "generate", "length": 16, "symbols": false }),
    );
    let after = reply(&mut host, json!({ "id": 12, "verb": "list" }));

    assert_eq!(
        before["entries"], after["entries"],
        "generate must not write (ADR-0002)"
    );
}

#[test]
fn a_mutating_verb_is_refused_and_changes_nothing() {
    let (_dir, mut host) = sample();
    let before = reply(&mut host, json!({ "id": 13, "verb": "list" }));

    // `insert` and `update` are deliberately absent: creating is gated on the
    // name being free (ADR-0006) and rewriting on the entry already existing
    // and a confirmation in the UI (ADR-0009). Everything here would move or
    // destroy what is in the store.
    for verb in ["edit", "rm", "mv", "cp", "delete", "init"] {
        let response = reply(
            &mut host,
            json!({ "id": 14, "verb": verb, "entry": "web/google.com", "password": "owned" }),
        );
        assert_eq!(response["ok"], json!(false), "{verb} was not refused");
        assert_eq!(
            error_code(&response),
            "read_only",
            "{verb} got the wrong code"
        );
    }

    let after = reply(&mut host, json!({ "id": 15, "verb": "list" }));
    assert_eq!(before["entries"], after["entries"]);
}

#[test]
fn insert_creates_an_entry_the_next_get_can_read_back() {
    let (_dir, mut host) = sample();
    let response = reply(
        &mut host,
        json!({
            "id": 20,
            "verb": "insert",
            "entry": "web/example.com",
            "secret": "hunter2",
            "fields": [
                { "key": "username", "value": "sana@example.com" },
                { "key": "url", "value": "https://example.com/login" },
            ],
        }),
    );
    assert_eq!(response["ok"], json!(true));
    assert_eq!(response["entry"], json!("web/example.com"));

    // Written in the shape the CLI writes, so `nopass show` and the popup
    // agree about what is in the file.
    let got = reply(
        &mut host,
        json!({ "id": 21, "verb": "get", "entry": "web/example.com" }),
    );
    assert_eq!(got["entry"]["secret"], json!("hunter2"));
    assert_eq!(field(&got["entry"], "username"), Some("sana@example.com"));
    assert_eq!(
        field(&got["entry"], "url"),
        Some("https://example.com/login")
    );

    // And it is a real entry, not just a readable file.
    let names = reply(&mut host, json!({ "id": 22, "verb": "list" }));
    assert!(
        names["entries"]
            .as_array()
            .expect("list returns an array")
            .contains(&json!("web/example.com")),
        "{names}"
    );
}

#[test]
fn insert_refuses_a_name_that_is_already_taken_rather_than_overwriting_it() {
    // The whole reason this verb is safe to expose: a compromised extension
    // must not be able to replace a login with one it knows (ADR-0006).
    let (_dir, mut host) = sample();
    let response = reply(
        &mut host,
        json!({
            "id": 23,
            "verb": "insert",
            "entry": "web/github.com",
            "secret": "owned",
        }),
    );
    assert_eq!(response["ok"], json!(false));
    assert_eq!(error_code(&response), "exists");

    let got = reply(
        &mut host,
        json!({ "id": 24, "verb": "get", "entry": "web/github.com" }),
    );
    assert_eq!(
        got["entry"]["secret"],
        json!("octocat-pw"),
        "the original password must survive a refused insert"
    );
}

#[test]
fn insert_is_refused_while_the_store_is_locked() {
    let (_dir, mut host) = sample_locked();
    let response = reply(
        &mut host,
        json!({
            "id": 25,
            "verb": "insert",
            "entry": "web/example.com",
            "secret": "hunter2",
        }),
    );
    assert_eq!(response["ok"], json!(false));
    assert_eq!(error_code(&response), "locked");

    let names = reply(&mut host, json!({ "id": 26, "verb": "list" }));
    assert!(
        !names["entries"]
            .as_array()
            .expect("list returns an array")
            .contains(&json!("web/example.com")),
        "a locked store must not have gained an entry: {names}"
    );
}

#[test]
fn a_line_break_in_a_field_cannot_forge_a_second_one() {
    // `url:` on its own line is what the popup and the dropdown match on, so
    // a password free to carry one could point a future fill at a site the
    // user never typed.
    let (_dir, mut host) = sample();
    let response = reply(
        &mut host,
        json!({
            "id": 27,
            "verb": "insert",
            "entry": "web/example.com",
            "secret": "hunter2\nurl: https://phish.example",
        }),
    );
    assert_eq!(response["ok"], json!(false));
    assert_eq!(error_code(&response), "bad_request");
}

#[test]
fn an_unknown_verb_is_refused_as_unsupported_rather_than_read_only() {
    let (_dir, mut host) = sample();
    let response = reply(&mut host, json!({ "id": 16, "verb": "telepathy" }));

    assert_eq!(error_code(&response), "unsupported_verb");
}

#[test]
fn a_malformed_request_is_refused_without_losing_the_id() {
    let (_dir, mut host) = sample();
    let response = reply(&mut host, json!({ "id": 17, "verb": "get", "entry": "" }));

    assert_eq!(response["id"], json!(17));
    assert_eq!(error_code(&response), "bad_request");
}

#[test]
fn a_request_with_no_usable_id_still_gets_an_answer() {
    let (_dir, mut host) = sample();
    let response = reply(&mut host, json!({ "verb": "nonsense" }));

    assert_eq!(response["ok"], json!(false));
    assert_eq!(response["id"], json!(0));
}

#[test]
fn insert_writes_a_card_in_the_shape_the_cli_would_have_written() {
    let (_dir, mut host) = sample();
    let response = reply(
        &mut host,
        json!({
            "id": 30,
            "verb": "insert",
            "entry": "cards/visa",
            "kind": "card",
            "secret": "4111111111111111",
            "fields": [
                { "key": "cardholder", "value": "Sana Qureshi" },
                { "key": "exp-month", "value": "04" },
                { "key": "exp-year", "value": "2029" },
            ],
        }),
    );
    assert_eq!(response["ok"], json!(true));

    let got = reply(
        &mut host,
        json!({ "id": 31, "verb": "get", "entry": "cards/visa" }),
    );
    assert_eq!(got["entry"]["kind"], json!("card"));
    assert_eq!(got["entry"]["secret"], json!("4111111111111111"));
    assert_eq!(field(&got["entry"], "cardholder"), Some("Sana Qureshi"));
    assert_eq!(field(&got["entry"], "exp-month"), Some("04"));
}

#[test]
fn insert_writes_an_identity_that_has_no_secret_at_all() {
    let (_dir, mut host) = sample();
    let response = reply(
        &mut host,
        json!({
            "id": 32,
            "verb": "insert",
            "entry": "me/home",
            "kind": "identity",
            "fields": [
                { "key": "given-name", "value": "Sana" },
                { "key": "country", "value": "Bangladesh" },
            ],
        }),
    );
    assert_eq!(response["ok"], json!(true));

    let got = reply(
        &mut host,
        json!({ "id": 33, "verb": "get", "entry": "me/home" }),
    );
    assert_eq!(got["entry"]["kind"], json!("identity"));
    assert_eq!(got["entry"]["secret"], json!(""));
    assert_eq!(field(&got["entry"], "given-name"), Some("Sana"));
}

#[test]
fn update_rewrites_one_field_and_leaves_the_rest_alone() {
    let (_dir, mut host) = sample();
    let response = reply(
        &mut host,
        json!({
            "id": 34,
            "verb": "update",
            "entry": "web/google.com",
            "fields": [{ "key": "username", "value": "someone@else" }],
        }),
    );
    assert_eq!(response["ok"], json!(true));
    assert_eq!(response["entry"], json!("web/google.com"));

    let got = reply(
        &mut host,
        json!({ "id": 35, "verb": "get", "entry": "web/google.com" }),
    );
    assert_eq!(field(&got["entry"], "username"), Some("someone@else"));
    assert_eq!(got["entry"]["secret"], json!("hunter2"));
    assert_eq!(field(&got["entry"], "url"), Some("https://google.com"));
    assert_eq!(
        field(&got["entry"], "totp"),
        Some("otpauth://totp/g?secret=JBSWY3DPEHPK3PXP"),
        "an update must not drop what it did not name"
    );
}

#[test]
fn update_keeps_a_line_this_host_does_not_understand() {
    let (dir, store) = store_with(&[(
        "web/example.com",
        "hunter2\nusername: sana\nfavourite-colour: green\n",
    )]);
    let mut host = Host::with_session(store, Session::new(dir.path().join("identity.txt")));

    reply(
        &mut host,
        json!({
            "id": 36,
            "verb": "update",
            "entry": "web/example.com",
            "secret": "new-password",
        }),
    );

    let body = body_of(dir.path(), "web/example.com");
    assert!(
        body.contains("favourite-colour: green"),
        "an unknown line was dropped: {body}"
    );
    assert!(body.starts_with("new-password\n"), "{body}");
}

#[test]
fn update_writes_a_field_under_the_spelling_the_entry_already_used() {
    let (dir, store) = store_with(&[("web/example.com", "hunter2\nemail: old@example.com\n")]);
    let mut host = Host::with_session(store, Session::new(dir.path().join("identity.txt")));

    reply(
        &mut host,
        json!({
            "id": 37,
            "verb": "update",
            "entry": "web/example.com",
            "fields": [{ "key": "username", "value": "new@example.com" }],
        }),
    );

    let body = body_of(dir.path(), "web/example.com");
    assert!(body.contains("email: new@example.com"), "{body}");
    assert!(
        !body.contains("username:"),
        "a second spelling appeared: {body}"
    );
}

#[test]
fn update_clears_a_field_written_empty() {
    let (_dir, mut host) = sample();
    reply(
        &mut host,
        json!({
            "id": 38,
            "verb": "update",
            "entry": "web/github.com",
            "fields": [{ "key": "username", "value": "" }],
        }),
    );

    let got = reply(
        &mut host,
        json!({ "id": 39, "verb": "get", "entry": "web/github.com" }),
    );
    assert_eq!(field(&got["entry"], "username"), None);
}

#[test]
fn update_refuses_an_entry_that_is_not_there_rather_than_creating_it() {
    let (_dir, mut host) = sample();
    let response = reply(
        &mut host,
        json!({
            "id": 40,
            "verb": "update",
            "entry": "web/nowhere",
            "secret": "hunter2",
        }),
    );

    assert_eq!(response["ok"], json!(false));
    assert_eq!(error_code(&response), "not_found");

    let names = reply(&mut host, json!({ "id": 41, "verb": "list" }));
    assert!(
        !names["entries"]
            .as_array()
            .expect("a list")
            .contains(&json!("web/nowhere")),
        "update created an entry: {names}"
    );
}

#[test]
fn update_is_refused_while_the_store_is_locked() {
    let (_dir, mut host) = sample_locked();
    let response = reply(
        &mut host,
        json!({
            "id": 42,
            "verb": "update",
            "entry": "web/github.com",
            "secret": "owned",
        }),
    );

    assert_eq!(response["ok"], json!(false));
    assert_eq!(error_code(&response), "locked");
}

#[test]
fn items_lists_a_kind_with_a_hint_that_is_never_a_secret() {
    let (_dir, mut host) = sample();
    reply(
        &mut host,
        json!({
            "id": 43,
            "verb": "insert",
            "entry": "cards/visa",
            "kind": "card",
            "secret": "4111111111111111",
            "fields": [{ "key": "brand", "value": "Visa" }],
        }),
    );

    let response = reply(
        &mut host,
        json!({ "id": 44, "verb": "items", "kinds": ["card"] }),
    );
    let items = response["items"].as_array().expect("a list of items");
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0]["name"], json!("cards/visa"));
    assert_eq!(items[0]["kind"], json!("card"));

    let hint = items[0]["hint"].as_str().expect("a hint");
    assert!(hint.contains("1111"), "the last four are the point: {hint}");
    assert!(
        !response.to_string().contains("4111111111111111"),
        "items leaked a card number"
    );
}

#[test]
fn items_with_no_kinds_covers_the_whole_store() {
    let (_dir, mut host) = sample();
    let response = reply(&mut host, json!({ "id": 45, "verb": "items" }));

    let items = response["items"].as_array().expect("a list of items");
    assert_eq!(items.len(), 4, "{items:?}");
    assert!(items.iter().all(|item| item["kind"] == json!("login")));
    assert!(
        !response.to_string().contains("hunter2"),
        "items leaked a password"
    );
}
