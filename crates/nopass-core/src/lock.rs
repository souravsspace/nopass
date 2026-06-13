//! Locked identities: the age secret key encrypted at rest behind one or
//! more authentication "slots". Each slot is an independent age (scrypt)
//! ciphertext of the *same* identity secret, openable by a different secret
//! — a user passphrase, or a high-entropy secret released by a platform
//! authenticator (e.g. macOS Touch ID via the Keychain). This mirrors the
//! key-slot model of disk encryption: unlocking any one slot recovers the
//! identity, and slots can be added or removed independently.

use age::secrecy::SecretString;
use base64::Engine;

use crate::error::{Error, Result};

/// First line of a locked identity file. Its presence is what distinguishes
/// a locked identity from a plaintext one.
pub const LOCK_HEADER: &str = "# nopass-locked v1";

const SLOT_PASSPHRASE: &str = "slot-passphrase:";
const SLOT_KEYCHAIN: &str = "slot-keychain:";

fn b64() -> base64::engine::general_purpose::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

/// A locked identity, holding the encrypted slots present on disk.
#[derive(Debug, Default, Clone)]
pub struct LockedIdentity {
    /// age ciphertext of the identity under the user's passphrase.
    pub passphrase_slot: Option<Vec<u8>>,
    /// age ciphertext of the identity under a Keychain-held secret.
    pub keychain_slot: Option<Vec<u8>>,
}

impl LockedIdentity {
    /// Whether `contents` look like a locked identity file (vs. plaintext).
    pub fn is_locked_file(contents: &str) -> bool {
        contents.trim_start().starts_with(LOCK_HEADER)
    }

    pub fn has_passphrase(&self) -> bool {
        self.passphrase_slot.is_some()
    }

    pub fn has_keychain(&self) -> bool {
        self.keychain_slot.is_some()
    }

    /// Parse the on-disk text format.
    pub fn parse(contents: &str) -> Result<Self> {
        if !Self::is_locked_file(contents) {
            return Err(Error::AuthFailed("not a locked identity file".into()));
        }
        let mut locked = LockedIdentity::default();
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line == LOCK_HEADER || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix(SLOT_PASSPHRASE) {
                locked.passphrase_slot = Some(decode_slot(rest)?);
            } else if let Some(rest) = line.strip_prefix(SLOT_KEYCHAIN) {
                locked.keychain_slot = Some(decode_slot(rest)?);
            }
        }
        if locked.passphrase_slot.is_none() && locked.keychain_slot.is_none() {
            return Err(Error::AuthFailed("locked identity has no slots".into()));
        }
        Ok(locked)
    }

    /// Render to the on-disk text format.
    pub fn serialize(&self) -> String {
        let mut out = format!("{LOCK_HEADER}\n");
        out.push_str("# Encrypted identity — unlock with passkey/passphrase.\n");
        if let Some(slot) = &self.passphrase_slot {
            out.push_str(&format!("{SLOT_PASSPHRASE} {}\n", b64().encode(slot)));
        }
        if let Some(slot) = &self.keychain_slot {
            out.push_str(&format!("{SLOT_KEYCHAIN} {}\n", b64().encode(slot)));
        }
        out
    }
}

fn decode_slot(s: &str) -> Result<Vec<u8>> {
    b64()
        .decode(s.trim())
        .map_err(|e| Error::AuthFailed(format!("bad slot encoding: {e}")))
}

/// Encrypt `identity_secret` (an `AGE-SECRET-KEY-...` string) under `secret`,
/// producing one slot's age ciphertext.
pub fn encrypt_slot(identity_secret: &str, secret: &SecretString) -> Result<Vec<u8>> {
    let recipient = age::scrypt::Recipient::new(secret.clone());
    age::encrypt(&recipient, identity_secret.as_bytes())
        .map_err(|e| Error::AuthFailed(format!("could not seal slot: {e}")))
}

/// Decrypt a slot's age ciphertext with `secret`, recovering the identity
/// secret string. A wrong secret surfaces as [`Error::AuthFailed`].
pub fn decrypt_slot(ciphertext: &[u8], secret: &SecretString) -> Result<String> {
    let identity = age::scrypt::Identity::new(secret.clone());
    let plaintext = age::decrypt(&identity, ciphertext)
        .map_err(|e| Error::AuthFailed(format!("wrong passphrase or corrupt slot: {e}")))?;
    String::from_utf8(plaintext)
        .map_err(|e| Error::AuthFailed(format!("decrypted slot is not valid UTF-8: {e}")))
}

/// Strategy for turning a [`LockedIdentity`] back into a usable secret key.
/// The core ships only the passphrase-aware pieces; richer unlockers (Touch
/// ID, security keys) live in front-ends and implement this trait.
pub trait Unlocker: Send + Sync {
    /// Return the decrypted `AGE-SECRET-KEY-...` string, prompting the user
    /// for whatever factor is required.
    fn unlock(&self, locked: &LockedIdentity) -> Result<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pass(s: &str) -> SecretString {
        SecretString::from(s.to_owned())
    }

    #[test]
    fn slot_roundtrip() {
        let secret = "AGE-SECRET-KEY-1EXAMPLE";
        let ct = encrypt_slot(secret, &pass("hunter2")).unwrap();
        assert_eq!(decrypt_slot(&ct, &pass("hunter2")).unwrap(), secret);
    }

    #[test]
    fn wrong_passphrase_fails() {
        let ct = encrypt_slot("AGE-SECRET-KEY-1EXAMPLE", &pass("right")).unwrap();
        assert!(matches!(
            decrypt_slot(&ct, &pass("wrong")),
            Err(Error::AuthFailed(_))
        ));
    }

    #[test]
    fn parse_serialize_roundtrip() {
        let locked = LockedIdentity {
            passphrase_slot: Some(b"abc".to_vec()),
            keychain_slot: Some(b"xyz".to_vec()),
        };
        let text = locked.serialize();
        assert!(LockedIdentity::is_locked_file(&text));
        let parsed = LockedIdentity::parse(&text).unwrap();
        assert_eq!(parsed.passphrase_slot.as_deref(), Some(&b"abc"[..]));
        assert_eq!(parsed.keychain_slot.as_deref(), Some(&b"xyz"[..]));
    }

    #[test]
    fn parse_passphrase_only() {
        let locked = LockedIdentity {
            passphrase_slot: Some(b"only".to_vec()),
            keychain_slot: None,
        };
        let parsed = LockedIdentity::parse(&locked.serialize()).unwrap();
        assert!(parsed.has_passphrase());
        assert!(!parsed.has_keychain());
    }

    #[test]
    fn plaintext_is_not_a_lock_file() {
        assert!(!LockedIdentity::is_locked_file("AGE-SECRET-KEY-1XXXX\n"));
        assert!(LockedIdentity::parse("AGE-SECRET-KEY-1XXXX\n").is_err());
    }

    #[test]
    fn empty_lock_rejected() {
        let text = format!("{LOCK_HEADER}\n# nothing here\n");
        assert!(LockedIdentity::parse(&text).is_err());
    }

    #[test]
    fn full_lock_unlock_via_passphrase_slot() {
        // Simulate the real flow: a generated identity, locked into a
        // passphrase slot, then recovered.
        let identity = age::x25519::Identity::generate();
        use age::secrecy::ExposeSecret;
        let secret = identity.to_string();
        let secret = secret.expose_secret();

        let locked = LockedIdentity {
            passphrase_slot: Some(encrypt_slot(secret, &pass("master")).unwrap()),
            keychain_slot: None,
        };
        let text = locked.serialize();

        let reparsed = LockedIdentity::parse(&text).unwrap();
        let recovered =
            decrypt_slot(reparsed.passphrase_slot.as_ref().unwrap(), &pass("master")).unwrap();
        assert_eq!(&recovered, secret);
        // And it parses back into a working identity.
        assert!(recovered.parse::<age::x25519::Identity>().is_ok());
    }
}
