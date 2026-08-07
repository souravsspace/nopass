//! Reading an entry body.
//!
//! nopass inherits the shape `pass` established: the first line is the
//! password and everything after it is whatever the user wanted to keep with
//! it. Nothing about that is schema'd, so this parser recognises the
//! conventional keys and leaves the rest alone.

/// The fields the extension can use out of an entry body.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Fields {
    pub password: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub totp: Option<String>,
}

const USERNAME_KEYS: &[&str] = &["username", "user", "login", "email"];
const URL_KEYS: &[&str] = &["url", "website", "site"];
const TOTP_KEYS: &[&str] = &["totp", "otp", "otpauth", "otp_secret"];

pub fn parse(body: &str) -> Fields {
    let mut lines = body.lines();
    let mut fields = Fields {
        password: lines.next().unwrap_or_default().trim_end().to_string(),
        ..Fields::default()
    };

    for line in lines {
        let line = line.trim();

        // A bare `otpauth://` URI is the common way to paste a TOTP secret.
        if line.starts_with("otpauth://") {
            fields.totp.get_or_insert_with(|| line.to_string());
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        if value.is_empty() {
            continue;
        }

        let slot = if USERNAME_KEYS.contains(&key.as_str()) {
            &mut fields.username
        } else if URL_KEYS.contains(&key.as_str()) {
            &mut fields.url
        } else if TOTP_KEYS.contains(&key.as_str()) {
            &mut fields.totp
        } else {
            continue;
        };

        // First occurrence wins, so a stray later line cannot shadow the
        // value the user put at the top.
        slot.get_or_insert_with(|| value.to_string());
    }

    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_line_is_the_password() {
        assert_eq!(parse("hunter2\nusername: sana\n").password, "hunter2");
    }

    #[test]
    fn an_entry_may_be_nothing_but_a_password() {
        let fields = parse("hunter2\n");
        assert_eq!(fields.password, "hunter2");
        assert_eq!(fields.username, None);
    }

    #[test]
    fn an_empty_body_yields_an_empty_password() {
        assert_eq!(parse(""), Fields::default());
    }

    #[test]
    fn the_conventional_keys_are_recognised_whatever_their_case() {
        let fields = parse("pw\nUsername: sana\nURL: https://example.com\nTOTP: abc\n");
        assert_eq!(fields.username.as_deref(), Some("sana"));
        assert_eq!(fields.url.as_deref(), Some("https://example.com"));
        assert_eq!(fields.totp.as_deref(), Some("abc"));
    }

    #[test]
    fn a_bare_otpauth_uri_is_a_totp() {
        let fields = parse("pw\notpauth://totp/x?secret=ABC\n");
        assert_eq!(fields.totp.as_deref(), Some("otpauth://totp/x?secret=ABC"));
    }

    #[test]
    fn a_url_value_keeps_the_colon_in_its_scheme() {
        assert_eq!(parse("pw\nurl: https://a.b/c\n").url.as_deref(), Some("https://a.b/c"));
    }

    #[test]
    fn an_unknown_key_is_left_alone() {
        let fields = parse("pw\nnotes: bought in 2019\n");
        assert_eq!(fields.username, None);
        assert_eq!(fields.url, None);
    }

    #[test]
    fn the_first_occurrence_wins() {
        assert_eq!(parse("pw\nuser: first\nuser: second\n").username.as_deref(), Some("first"));
    }

    #[test]
    fn a_windows_line_ending_does_not_end_up_in_the_password() {
        assert_eq!(parse("hunter2\r\nuser: sana\r\n").password, "hunter2");
    }
}
