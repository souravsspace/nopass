//! Between an entry on disk and an entry on the wire.
//!
//! The parsing itself lives in `nopass-core`, so the CLI and this host read
//! the same bodies by the same rules. What is here is the translation: the
//! wire speaks one canonical key per field, and a body speaks whichever
//! spelling its author used.
//!
//! That translation is the reason the extension needs no alias table. It also
//! means a field it has never heard of still reaches it — an unknown `key:`
//! line travels as itself, so a newer nopass and an older popup can look at
//! the same entry.

use anyhow::{bail, Result};
use nopass_core::fields;
use nopass_core::record::{Kind as RecordKind, Line, Record, TYPE_KEY};

use crate::proto::{Field, Kind, Secret};

/// How much of a card number a row may show. The last four digits are what
/// the issuer itself prints on a receipt.
const CARD_TAIL: usize = 4;

/// One entry as the wire describes it.
pub fn to_secret(name: String, record: &Record) -> Secret {
    let kind = Kind::from_record_kind(&record.kind());
    Secret {
        name,
        kind,
        secret: record.primary.clone(),
        fields: to_fields(kind, record),
    }
}

/// Every field of `record`, canonical keys first, then whatever else the body
/// carried that can be named on the wire.
pub fn to_fields(kind: Kind, record: &Record) -> Vec<Field> {
    let mut fields = Vec::new();
    let mut seen: Vec<&str> = Vec::new();

    for spec in fields::spec_for(&kind.as_record_kind()) {
        if let Some(value) = record.get(spec.aliases) {
            fields.push(Field {
                key: spec.key.to_string(),
                value: value.to_string(),
            });
            seen.extend_from_slice(spec.aliases);
        }
    }

    // A bare `otpauth://` URI is a TOTP secret nobody wrote a key for.
    if kind == Kind::Login && !fields.iter().any(|field| field.key == "totp") {
        if let Some(uri) = record.raw_starting_with("otpauth://") {
            fields.push(Field {
                key: "totp".to_string(),
                value: uri,
            });
        }
    }

    for line in &record.lines {
        let Line::Pair(key, value) = line else {
            continue;
        };
        // `type:` is the kind, which travels in its own slot, and a key the
        // wire cannot spell would be refused by the far end.
        if key == TYPE_KEY || seen.contains(&key.as_str()) || !is_field_key(key) {
            continue;
        }
        if fields.iter().any(|field| field.key == *key) {
            continue;
        }
        fields.push(Field {
            key: key.clone(),
            value: value.clone(),
        });
    }

    fields
}

/// Write a secret and a set of fields onto `record`.
///
/// Each field goes in under the spelling the entry already uses, so an edit
/// leaves a hand-written `email:` line saying `email:`. An empty value clears
/// the field. Every line the caller did not name is left exactly where it was,
/// which is what makes an update from an older client safe (ADR-0009).
pub fn apply(record: &mut Record, secret: Option<&str>, fields: &[Field]) -> Result<()> {
    if let Some(secret) = secret {
        record.set_primary(secret)?;
    }

    for field in fields {
        if field.key == TYPE_KEY {
            bail!("the kind is not a field");
        }
        match fields::aliases_for(&field.key) {
            Some(aliases) => record.set_alias(&field.key, aliases, &field.value)?,
            // A key this build does not know is still the caller's to write:
            // it names a line in their own entry.
            None => record.set(&field.key, &field.value)?,
        }
    }
    Ok(())
}

/// What a row for this entry should read, given it may carry no secret.
pub fn hint(kind: Kind, record: &Record) -> Option<String> {
    match kind {
        Kind::Card => Some(card_hint(record)),
        Kind::Identity => identity_hint(record),
        Kind::Login => record.get(fields::USERNAME_KEYS).map(str::to_string),
        Kind::Passkey => record
            .get(fields::PASSKEY_RP_KEYS)
            .map(str::to_string)
            .or_else(|| record.get(fields::PASSKEY_USER_KEYS).map(str::to_string)),
    }
}

/// `Visa •••• 4242`. The brand if the entry names one, and never more of the
/// number than a receipt would print.
fn card_hint(record: &Record) -> String {
    let card = fields::card(record);
    let digits: String = card.number.chars().filter(char::is_ascii_digit).collect();
    let tail = if digits.len() > CARD_TAIL {
        digits[digits.len() - CARD_TAIL..].to_string()
    } else {
        digits
    };

    match card.brand {
        Some(brand) if !tail.is_empty() => format!("{brand} •••• {tail}"),
        Some(brand) => brand,
        None if !tail.is_empty() => format!("•••• {tail}"),
        None => String::new(),
    }
}

fn identity_hint(record: &Record) -> Option<String> {
    let identity = fields::identity(record);
    let name = [identity.given_name, identity.family_name]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    if !name.is_empty() {
        return Some(name);
    }
    identity.email.or(identity.city)
}

fn is_field_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The record kind a wire kind writes. Kept here so `Kind` stays a wire type.
pub fn record_kind(kind: Kind) -> RecordKind {
    kind.as_record_kind()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value<'a>(fields: &'a [Field], key: &str) -> Option<&'a str> {
        fields
            .iter()
            .find(|field| field.key == key)
            .map(|field| field.value.as_str())
    }

    #[test]
    fn a_login_travels_as_its_canonical_fields() {
        let record = Record::parse("hunter2\nuser: sana\nwebsite: https://a.b\n");
        let secret = to_secret("web/a.b".to_string(), &record);

        assert_eq!(secret.kind, Kind::Login);
        assert_eq!(secret.secret, "hunter2");
        assert_eq!(value(&secret.fields, "username"), Some("sana"));
        assert_eq!(value(&secret.fields, "url"), Some("https://a.b"));
    }

    #[test]
    fn a_bare_otpauth_line_travels_as_a_totp_field() {
        let record = Record::parse("pw\notpauth://totp/x?secret=ABC\n");
        let fields = to_fields(Kind::Login, &record);

        assert_eq!(value(&fields, "totp"), Some("otpauth://totp/x?secret=ABC"));
    }

    #[test]
    fn a_line_this_host_does_not_claim_still_reaches_the_extension() {
        let record = Record::parse("pw\nnotes: bought in 2019\n");
        let fields = to_fields(Kind::Login, &record);

        assert_eq!(value(&fields, "notes"), Some("bought in 2019"));
    }

    #[test]
    fn the_type_line_never_travels_as_a_field() {
        let record = Record::parse("4111\ntype: card\n");
        let fields = to_fields(Kind::Card, &record);

        assert!(value(&fields, "type").is_none(), "{fields:?}");
    }

    #[test]
    fn a_key_the_wire_cannot_spell_is_left_behind_rather_than_mangled() {
        let record = Record::parse("pw\nFavourite Colour: green\n");
        let fields = to_fields(Kind::Login, &record);

        assert!(
            fields.iter().all(|field| field.key != "favourite colour"),
            "{fields:?}"
        );
    }

    #[test]
    fn applying_a_field_keeps_the_spelling_the_entry_already_used() {
        let mut record = Record::parse("pw\nemail: old@example.com\n");
        apply(
            &mut record,
            None,
            &[Field {
                key: "username".to_string(),
                value: "new@example.com".to_string(),
            }],
        )
        .expect("the field applies");

        assert_eq!(record.render(), "pw\nemail: new@example.com\n");
    }

    #[test]
    fn applying_an_empty_value_clears_the_field() {
        let mut record = Record::parse("pw\nusername: sana\nurl: https://a.b\n");
        apply(
            &mut record,
            None,
            &[Field {
                key: "username".to_string(),
                value: String::new(),
            }],
        )
        .expect("the field applies");

        assert_eq!(record.render(), "pw\nurl: https://a.b\n");
    }

    #[test]
    fn applying_refuses_to_write_the_type_line() {
        let mut record = Record::parse("pw\n");
        assert!(apply(
            &mut record,
            None,
            &[Field {
                key: TYPE_KEY.to_string(),
                value: "card".to_string(),
            }],
        )
        .is_err());
    }

    #[test]
    fn a_card_hint_shows_the_brand_and_the_last_four_only() {
        let record = Record::parse("4111 1111 1111 4242\ntype: card\nbrand: Visa\n");
        let hint = hint(Kind::Card, &record).expect("a card has a hint");

        assert_eq!(hint, "Visa •••• 4242");
        assert!(!hint.contains("4111"), "{hint}");
    }

    #[test]
    fn a_card_with_no_brand_is_still_recognisable() {
        let record = Record::parse("4111111111114242\ntype: card\n");
        assert_eq!(hint(Kind::Card, &record).as_deref(), Some("•••• 4242"));
    }

    #[test]
    fn an_identity_is_named_by_its_person() {
        let record = Record::parse("\ntype: identity\ngiven-name: Sana\nfamily-name: Qureshi\n");
        assert_eq!(
            hint(Kind::Identity, &record).as_deref(),
            Some("Sana Qureshi")
        );
    }

    #[test]
    fn an_identity_with_no_name_falls_back_to_something_it_has() {
        let record = Record::parse("\ntype: identity\nemail: sana@example.com\n");
        assert_eq!(
            hint(Kind::Identity, &record).as_deref(),
            Some("sana@example.com")
        );
    }

    #[test]
    fn a_login_row_reads_as_its_username() {
        let record = Record::parse("pw\nusername: sana\n");
        assert_eq!(hint(Kind::Login, &record).as_deref(), Some("sana"));
    }

    #[test]
    fn what_is_written_reads_back_the_same() {
        let mut record = Record::new(record_kind(Kind::Card));
        apply(
            &mut record,
            Some("4111111111111111"),
            &[
                Field {
                    key: "cardholder".to_string(),
                    value: "Sana Q".to_string(),
                },
                Field {
                    key: "exp-month".to_string(),
                    value: "04".to_string(),
                },
            ],
        )
        .expect("the card applies");

        let back = Record::parse(&record.render());
        let secret = to_secret("cards/visa".to_string(), &back);
        assert_eq!(secret.kind, Kind::Card);
        assert_eq!(secret.secret, "4111111111111111");
        assert_eq!(value(&secret.fields, "cardholder"), Some("Sana Q"));
        assert_eq!(value(&secret.fields, "exp-month"), Some("04"));
    }
}
