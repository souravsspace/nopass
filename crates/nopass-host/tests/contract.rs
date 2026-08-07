//! The other half of the contract test.
//!
//! `packages/protocol` validates the same fixture file with Zod. If a shape
//! changes on one side only, one of the two suites goes red.

use nopass_host::proto::{self, PROTOCOL_VERSION};
use serde_json::Value;

fn fixtures() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/protocol/fixtures/messages.json"
    );
    let text = std::fs::read_to_string(path).expect("the fixture file must exist");
    serde_json::from_str(&text).expect("the fixture file must be JSON")
}

fn cases(side: &str) -> Vec<(String, bool, Value)> {
    fixtures()[side]
        .as_array()
        .expect("each side is an array of cases")
        .iter()
        .map(|case| {
            (
                case["name"]
                    .as_str()
                    .expect("a case has a name")
                    .to_string(),
                case["valid"].as_bool().expect("a case has a verdict"),
                case["json"].clone(),
            )
        })
        .collect()
}

#[test]
fn the_fixture_file_agrees_with_the_version_we_announce() {
    assert_eq!(
        fixtures()["protocolVersion"].as_u64(),
        Some(PROTOCOL_VERSION as u64)
    );
}

#[test]
fn every_side_covers_both_verdicts() {
    for side in ["requests", "responses"] {
        let cases = cases(side);
        assert!(
            cases.iter().any(|(_, valid, _)| *valid),
            "{side} has no accepted case"
        );
        assert!(
            cases.iter().any(|(_, valid, _)| !*valid),
            "{side} has no rejected case"
        );
    }
}

#[test]
fn requests_match_the_fixtures() {
    for (name, valid, json) in cases("requests") {
        let parsed = proto::parse_request(&json);
        assert_eq!(parsed.is_ok(), valid, "{name}: {parsed:?}");

        if let Ok(request) = parsed {
            let back = serde_json::to_value(&request).expect("a request re-serialises");
            assert_eq!(back, json, "{name} did not round-trip");
        }
    }
}

#[test]
fn responses_match_the_fixtures() {
    for (name, valid, json) in cases("responses") {
        let parsed = proto::parse_response(&json);
        assert_eq!(parsed.is_ok(), valid, "{name}: {parsed:?}");

        if let Ok(response) = parsed {
            let back = serde_json::to_value(&response).expect("a response re-serialises");
            assert_eq!(back, json, "{name} did not round-trip");
        }
    }
}

#[test]
fn no_mutating_verb_can_be_expressed_on_this_wire() {
    for verb in [
        "insert", "edit", "rm", "mv", "cp", "delete", "remove", "init",
    ] {
        let json = serde_json::json!({ "id": 1, "verb": verb, "entry": "web/example.com" });
        assert!(
            proto::parse_request(&json).is_err(),
            "{verb} must not parse: the host is read-only (ADR-0002)"
        );
    }
}
