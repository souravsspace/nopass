//! What an entry *is*, beyond a password.
//!
//! An entry body has always been one line of secret followed by free-form
//! `key: value` lines, and everything that did not recognise a key ignored it.
//! That is the whole extension point: a `type:` line names what the entry
//! holds, and each kind claims its own keys. An entry with no `type:` is a
//! login, which is what every entry written before this module was.
//!
//! Two rules make it safe to rewrite an entry that something else authored:
//!
//! - **Nothing is dropped.** A line this build does not understand is carried
//!   through a parse and a render unchanged, in its original position. A newer
//!   nopass, or a hand-written note, survives an edit made here.
//! - **A value is one line.** A value free to carry a newline could forge a
//!   second field — a `url:` line pointing somewhere the user never typed,
//!   which is a phishing primitive rather than a formatting bug (ADR-0006).

use crate::error::{Error, Result};

/// The key that names an entry's kind. Absent means [`Kind::Login`].
pub const TYPE_KEY: &str = "type";

/// What an entry holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// A password, optionally with a username and a site. The original shape.
    Login,
    /// A payment card. The first line is the card number.
    Card,
    /// A person: name, address, contact details. No secret on the first line.
    Identity,
    /// A WebAuthn credential. The first line is its private key.
    Passkey,
    /// A kind this build has never heard of, kept verbatim so a newer nopass
    /// can still read what an older one has rewritten.
    Other(String),
}

impl Kind {
    pub fn as_str(&self) -> &str {
        match self {
            Kind::Login => "login",
            Kind::Card => "card",
            Kind::Identity => "identity",
            Kind::Passkey => "passkey",
            Kind::Other(name) => name,
        }
    }

    fn from_value(value: &str) -> Kind {
        match value.trim().to_ascii_lowercase().as_str() {
            "login" | "password" => Kind::Login,
            "card" | "credit-card" => Kind::Card,
            "identity" | "profile" => Kind::Identity,
            "passkey" | "webauthn" => Kind::Passkey,
            other => Kind::Other(other.to_string()),
        }
    }
}

/// One line of an entry body, after the first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// A `key: value` pair. The key is stored lower-cased; the value is not.
    Pair(String, String),
    /// Anything else — a bare `otpauth://` URI, a blank line, a note someone
    /// typed. Preserved exactly.
    Raw(String),
}

/// A parsed entry body.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Record {
    /// The first line. A password, a card number, a private key, or empty.
    pub primary: String,
    /// Every line after it, in the order they were written.
    pub lines: Vec<Line>,
}

impl Record {
    /// An empty record of `kind`, ready to have fields set on it.
    pub fn new(kind: Kind) -> Record {
        let mut record = Record::default();
        if kind != Kind::Login {
            record
                .lines
                .push(Line::Pair(TYPE_KEY.to_string(), kind.as_str().to_string()));
        }
        record
    }

    pub fn parse(body: &str) -> Record {
        let mut lines = body.lines();
        let primary = lines.next().unwrap_or_default().trim_end().to_string();
        Record {
            primary,
            lines: lines.map(parse_line).collect(),
        }
    }

    /// The body as it goes back to disk. Round-trips whatever was parsed.
    pub fn render(&self) -> String {
        let mut body = String::with_capacity(self.primary.len() + 64);
        body.push_str(&self.primary);
        body.push('\n');
        for line in &self.lines {
            match line {
                Line::Pair(key, value) => {
                    body.push_str(key);
                    body.push_str(": ");
                    body.push_str(value);
                    body.push('\n');
                }
                Line::Raw(text) => {
                    body.push_str(text);
                    body.push('\n');
                }
            }
        }
        body
    }

    pub fn kind(&self) -> Kind {
        self.get(&[TYPE_KEY]).map_or(Kind::Login, Kind::from_value)
    }

    /// The first value written under any of `keys`.
    ///
    /// First occurrence wins, so a stray later line cannot shadow what the
    /// user put at the top, and the aliases are tried in the order the caller
    /// listed them only insofar as the body did.
    pub fn get(&self, keys: &[&str]) -> Option<&str> {
        self.lines.iter().find_map(|line| match line {
            Line::Pair(key, value) if keys.contains(&key.as_str()) => Some(value.as_str()),
            _ => None,
        })
    }

    /// The first line that is not a `key: value` pair and begins with
    /// `prefix` — how a pasted `otpauth://` URI is found.
    pub fn raw_starting_with(&self, prefix: &str) -> Option<String> {
        self.lines.iter().find_map(|line| match line {
            Line::Raw(text) if text.starts_with(prefix) => Some(text.clone()),
            _ => None,
        })
    }

    /// Write `key`, replacing the first line that already holds it and leaving
    /// every other line where it is. An empty value removes the field rather
    /// than writing a blank one.
    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        check_one_line(value)?;
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();

        if value.is_empty() {
            self.remove(&key);
            return Ok(());
        }

        for line in &mut self.lines {
            if let Line::Pair(existing, slot) = line {
                if *existing == key {
                    *slot = value.to_string();
                    return Ok(());
                }
            }
        }
        self.lines.push(Line::Pair(key, value.to_string()));
        Ok(())
    }

    /// Drop every line written under `key`.
    pub fn remove(&mut self, key: &str) {
        let key = key.trim().to_ascii_lowercase();
        self.lines
            .retain(|line| !matches!(line, Line::Pair(existing, _) if *existing == key));
    }

    pub fn set_primary(&mut self, value: &str) -> Result<()> {
        check_one_line(value)?;
        self.primary = value.to_string();
        Ok(())
    }
}

fn parse_line(line: &str) -> Line {
    let trimmed = line.trim();

    if let Some((key, value)) = trimmed.split_once(':') {
        let key = key.trim();
        let value = value.trim();
        // A bare `otpauth://…` URI is the common way to paste a TOTP secret.
        // The colon in a scheme is not a key separator, and rewriting one as
        // `otpauth: //totp/…` would hand back a URI nothing can read.
        let scheme = value.starts_with("//");
        if !(key.is_empty() || value.is_empty() || scheme || key.contains(char::is_whitespace)) {
            return Line::Pair(key.to_ascii_lowercase(), value.to_string());
        }
    }
    Line::Raw(trimmed.to_string())
}

fn check_one_line(value: &str) -> Result<()> {
    if value.contains(['\r', '\n']) {
        return Err(Error::Encrypt(
            "a field value must not contain a line break".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_with_no_type_line_is_a_login() {
        assert_eq!(
            Record::parse("hunter2\nusername: sana\n").kind(),
            Kind::Login
        );
    }

    #[test]
    fn the_type_line_names_the_kind() {
        assert_eq!(Record::parse("4111\ntype: card\n").kind(), Kind::Card);
        assert_eq!(Record::parse("\ntype: identity\n").kind(), Kind::Identity);
        assert_eq!(Record::parse("key\ntype: passkey\n").kind(), Kind::Passkey);
    }

    #[test]
    fn a_kind_this_build_does_not_know_is_kept_rather_than_guessed_at() {
        let record = Record::parse("x\ntype: ssh-key\n");
        assert_eq!(record.kind(), Kind::Other("ssh-key".to_string()));
        assert_eq!(record.render(), "x\ntype: ssh-key\n");
    }

    #[test]
    fn the_type_line_is_read_whatever_its_case() {
        assert_eq!(Record::parse("4111\nType: CARD\n").kind(), Kind::Card);
    }

    #[test]
    fn the_first_line_is_the_primary_secret() {
        assert_eq!(Record::parse("hunter2\nuser: sana\n").primary, "hunter2");
        assert_eq!(Record::parse("").primary, "");
    }

    #[test]
    fn an_identity_may_have_no_secret_at_all() {
        let record = Record::parse("\ntype: identity\ngiven-name: Sana\n");
        assert_eq!(record.primary, "");
        assert_eq!(record.get(&["given-name"]), Some("Sana"));
    }

    #[test]
    fn a_bare_otpauth_uri_survives_a_round_trip() {
        // The colon in its scheme is not a key separator, and rewriting it as
        // `otpauth: //totp/…` would hand back a URI nothing can read.
        let body = "pw\notpauth://totp/x?secret=ABC\n";
        let record = Record::parse(body);
        assert_eq!(
            record.lines,
            vec![Line::Raw("otpauth://totp/x?secret=ABC".to_string())]
        );
        assert_eq!(record.render(), body);
    }

    #[test]
    fn a_url_value_keeps_the_colon_in_its_scheme() {
        assert_eq!(
            Record::parse("pw\nurl: https://a.b/c\n").get(&["url"]),
            Some("https://a.b/c")
        );
    }

    #[test]
    fn a_line_this_build_does_not_understand_is_carried_through_untouched() {
        let body = "pw\nusername: sana\nfavourite-colour: green\n\na loose note\n";
        assert_eq!(Record::parse(body).render(), body);
    }

    #[test]
    fn an_edit_leaves_every_other_line_where_it_was() {
        let mut record = Record::parse("pw\nusername: sana\nnotes: keep me\nurl: https://a.b\n");
        record.set("username", "someone@else").expect("one line");

        assert_eq!(
            record.render(),
            "pw\nusername: someone@else\nnotes: keep me\nurl: https://a.b\n"
        );
    }

    #[test]
    fn setting_a_field_that_was_not_there_appends_it() {
        let mut record = Record::parse("pw\n");
        record.set("url", "https://a.b").expect("one line");
        assert_eq!(record.render(), "pw\nurl: https://a.b\n");
    }

    #[test]
    fn setting_an_empty_value_removes_the_field_rather_than_writing_a_blank() {
        let mut record = Record::parse("pw\nusername: sana\nurl: https://a.b\n");
        record.set("username", "  ").expect("one line");
        assert_eq!(record.render(), "pw\nurl: https://a.b\n");
    }

    #[test]
    fn a_key_is_matched_whatever_case_it_was_written_in() {
        let mut record = Record::parse("pw\nUsername: sana\n");
        assert_eq!(record.get(&["username"]), Some("sana"));
        record.set("USERNAME", "other").expect("one line");
        assert_eq!(record.render(), "pw\nusername: other\n");
    }

    #[test]
    fn the_first_occurrence_wins() {
        assert_eq!(
            Record::parse("pw\nuser: first\nuser: second\n").get(&["user"]),
            Some("first")
        );
    }

    #[test]
    fn aliases_are_searched_together() {
        assert_eq!(
            Record::parse("pw\nlogin: sana\n").get(&["username", "user", "login"]),
            Some("sana")
        );
    }

    #[test]
    fn removing_a_field_drops_every_line_that_held_it() {
        let mut record = Record::parse("pw\nuser: a\nurl: https://a.b\nuser: b\n");
        record.remove("user");
        assert_eq!(record.render(), "pw\nurl: https://a.b\n");
    }

    #[test]
    fn a_value_carrying_a_line_break_is_refused_rather_than_written() {
        let mut record = Record::parse("pw\n");
        assert!(record
            .set("notes", "one\nurl: https://evil.example")
            .is_err());
        assert!(record.set_primary("pw\nurl: https://evil.example").is_err());
        // Nothing was written on the way to refusing.
        assert_eq!(record.render(), "pw\n");
    }

    #[test]
    fn a_windows_line_ending_does_not_end_up_in_a_value() {
        let record = Record::parse("hunter2\r\nuser: sana\r\n");
        assert_eq!(record.primary, "hunter2");
        assert_eq!(record.get(&["user"]), Some("sana"));
    }

    #[test]
    fn a_new_record_carries_its_kind_and_a_login_does_not_need_to() {
        assert_eq!(Record::new(Kind::Login).render(), "\n");
        assert_eq!(Record::new(Kind::Card).render(), "\ntype: card\n");
    }

    #[test]
    fn what_is_rendered_parses_back_to_what_went_in() {
        let mut record = Record::new(Kind::Card);
        record.set_primary("4111111111111111").expect("one line");
        record.set("cardholder", "Sana Q").expect("one line");
        record.set("exp", "04/2029").expect("one line");

        let body = record.render();
        let back = Record::parse(&body);
        assert_eq!(back, record);
        assert_eq!(back.kind(), Kind::Card);
        assert_eq!(back.primary, "4111111111111111");
        assert_eq!(back.get(&["cardholder"]), Some("Sana Q"));
    }
}
