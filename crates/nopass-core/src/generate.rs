use rand::Rng;

use crate::error::{Error, Result};

/// Default generated password length (override with NOPASS_GENERATED_LENGTH).
pub const DEFAULT_LENGTH: usize = 25;

/// Character sets for the `[:punct:][:alnum:]` and `[:alnum:]`
/// POSIX classes over ASCII.
pub fn charset(no_symbols: bool) -> Vec<u8> {
    (0x21u8..=0x7e)
        .filter(|b| {
            if no_symbols {
                b.is_ascii_alphanumeric()
            } else {
                b.is_ascii_alphanumeric() || b.is_ascii_punctuation()
            }
        })
        .collect()
}

/// Generate a password of `length` characters drawn uniformly from `chars`
/// using the OS CSPRNG.
pub fn password(length: usize, chars: &[u8]) -> Result<String> {
    if length == 0 {
        return Err(Error::ZeroLength);
    }
    if chars.is_empty() {
        return Err(Error::Encrypt("empty character set".into()));
    }
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..length)
        .map(|_| chars[rng.random_range(0..chars.len())])
        .collect();
    Ok(String::from_utf8(bytes).expect("charset is ascii"))
}

/// Build a charset from a literal set of characters (NOPASS_CHARACTER_SET
/// override). POSIX class names are expanded; other characters taken verbatim.
pub fn charset_from_spec(spec: &str) -> Vec<u8> {
    type ClassPred = fn(u8) -> bool;
    let mut out: Vec<u8> = Vec::new();
    let mut rest = spec;
    while !rest.is_empty() {
        let classes: [(&str, ClassPred); 7] = [
            ("[:alnum:]", |b| b.is_ascii_alphanumeric()),
            ("[:alpha:]", |b| b.is_ascii_alphabetic()),
            ("[:digit:]", |b| b.is_ascii_digit()),
            ("[:punct:]", |b| b.is_ascii_punctuation()),
            ("[:lower:]", |b| b.is_ascii_lowercase()),
            ("[:upper:]", |b| b.is_ascii_uppercase()),
            ("[:graph:]", |b| b.is_ascii_graphic()),
        ];
        let expanded = classes.iter().find_map(|(name, pred)| {
            rest.strip_prefix(name)
                .map(|r| ((0x21u8..=0x7e).filter(|&b| pred(b)).collect::<Vec<_>>(), r))
        });
        match expanded {
            Some((chars, remaining)) => {
                out.extend(chars);
                rest = remaining;
            }
            None => {
                let ch = rest.chars().next().unwrap();
                if ch.is_ascii() {
                    out.push(ch as u8);
                }
                rest = &rest[ch.len_utf8()..];
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_requested_length() {
        let chars = charset(false);
        for len in [1, 19, 25, 100] {
            assert_eq!(password(len, &chars).unwrap().chars().count(), len);
        }
    }

    #[test]
    fn no_symbols_is_alnum_only() {
        let chars = charset(true);
        let pw = password(500, &chars).unwrap();
        assert!(pw.chars().all(|c| c.is_ascii_alphanumeric()), "{pw}");
    }

    #[test]
    fn zero_length_rejected() {
        assert!(matches!(
            password(0, &charset(false)),
            Err(Error::ZeroLength)
        ));
    }

    #[test]
    fn spec_expands_posix_classes() {
        assert_eq!(
            charset_from_spec("[:digit:]"),
            (b'0'..=b'9').collect::<Vec<_>>()
        );
        let mixed = charset_from_spec("ab[:digit:]");
        assert!(mixed.contains(&b'a') && mixed.contains(&b'7') && !mixed.contains(&b'z'));
    }

    #[test]
    fn passwords_differ() {
        let chars = charset(false);
        let a = password(25, &chars).unwrap();
        let b = password(25, &chars).unwrap();
        assert_ne!(a, b);
    }
}
