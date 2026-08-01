//! Locked identities: the age secret key encrypted at rest behind one or
//! more authentication "slots". Each slot is an independent age (scrypt)
//! ciphertext of the *same* identity secret, openable by a different secret
//! — a user passphrase, a high-entropy secret released by a platform
//! authenticator (e.g. macOS Touch ID via the Keychain), or the HMAC output
//! of a FIDO2 security key (a passkey). This mirrors the key-slot model of
//! disk encryption: unlocking any one slot recovers the identity, and slots
//! can be added or removed independently.

use base64::Engine;

pub use age::secrecy::SecretString;

use crate::error::{Error, Result};

/// First line of a locked identity file. Its presence is what distinguishes
/// a locked identity from a plaintext one.
pub const LOCK_HEADER: &str = "# nopass-locked v1";

const SLOT_PASSPHRASE: &str = "slot-passphrase:";
const SLOT_KEYCHAIN: &str = "slot-keychain:";
const SLOT_FIDO2: &str = "slot-fido2:";

/// scrypt work factor for slots whose secret is *already* a uniform 256-bit
/// key (a security key's HMAC output). scrypt exists to stretch low-entropy
/// passphrases; stretching a full-entropy key buys nothing but latency, and
/// a security-key unlock should feel instant. Passphrase slots keep age's
/// device-calibrated default.
const HIGH_ENTROPY_LOG_N: u8 = 10;

/// Length of a security key's `hmac-secret` output, and of the salt we feed
/// it — both fixed at 32 bytes by the CTAP2 extension.
pub const HMAC_SECRET_LEN: usize = 32;

fn b64() -> base64::engine::general_purpose::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

/// One FIDO2 security-key slot: everything needed to ask an authenticator to
/// re-derive the key that opens [`Self::ciphertext`].
///
/// None of these fields are secret. The credential id and salt are the public
/// halves of the CTAP2 `hmac-secret` protocol: only the authenticator that
/// created the credential holds the per-credential seed that turns the salt
/// back into the unlocking key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fido2Slot {
    /// Human-chosen name, so `passkey status` and `passkey remove-key` can
    /// tell one key from another. Restricted to [`is_valid_label`].
    pub label: String,
    /// WebAuthn relying-party id the credential was created under.
    pub rp_id: String,
    /// Credential id returned by the authenticator at enrollment.
    pub credential_id: Vec<u8>,
    /// Random per-slot salt fed to the `hmac-secret` extension.
    pub salt: [u8; HMAC_SECRET_LEN],
    /// Whether the credential requires user verification (a PIN) as well as
    /// a touch.
    pub requires_pin: bool,
    /// age ciphertext of the identity under the HMAC output.
    pub ciphertext: Vec<u8>,
}

/// Labels name slots on the command line and are written verbatim into the
/// identity file, so keep them to an unambiguous, space-free charset.
pub fn is_valid_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 32
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// A locked identity, holding the encrypted slots present on disk.
#[derive(Debug, Default, Clone)]
pub struct LockedIdentity {
    /// age ciphertext of the identity under the user's passphrase.
    pub passphrase_slot: Option<Vec<u8>>,
    /// age ciphertext of the identity under a Keychain-held secret.
    pub keychain_slot: Option<Vec<u8>>,
    /// One entry per enrolled security key; any of them can unlock.
    pub fido2_slots: Vec<Fido2Slot>,
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

    pub fn has_fido2(&self) -> bool {
        !self.fido2_slots.is_empty()
    }

    /// Total number of usable slots — the caller's guard against writing an
    /// identity nothing can open.
    pub fn slot_count(&self) -> usize {
        usize::from(self.has_passphrase())
            + usize::from(self.has_keychain())
            + self.fido2_slots.len()
    }

    pub fn fido2_slot(&self, label: &str) -> Option<&Fido2Slot> {
        self.fido2_slots.iter().find(|s| s.label == label)
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
            } else if let Some(rest) = line.strip_prefix(SLOT_FIDO2) {
                locked.fido2_slots.push(parse_fido2_slot(rest)?);
            }
            // Anything else is a slot type this build doesn't know about.
            // Ignore it so a newer nopass can add slots without making the
            // file unreadable to an older one.
        }
        if locked.slot_count() == 0 {
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
        for slot in &self.fido2_slots {
            out.push_str(&format!(
                "{SLOT_FIDO2} name={} rp={} pin={} cred={} salt={} data={}\n",
                slot.label,
                slot.rp_id,
                if slot.requires_pin { "yes" } else { "no" },
                b64().encode(&slot.credential_id),
                b64().encode(slot.salt),
                b64().encode(&slot.ciphertext),
            ));
        }
        out
    }
}

fn decode_slot(s: &str) -> Result<Vec<u8>> {
    b64()
        .decode(s.trim())
        .map_err(|e| Error::AuthFailed(format!("bad slot encoding: {e}")))
}

/// Parse the `name=… rp=… pin=… cred=… salt=… data=…` tail of a
/// `slot-fido2:` line. Every field is required; base64 never contains
/// whitespace, so splitting on it is unambiguous.
fn parse_fido2_slot(rest: &str) -> Result<Fido2Slot> {
    let bad = |what: &str| Error::AuthFailed(format!("malformed security-key slot: {what}"));

    let mut label = None;
    let mut rp_id = None;
    let mut requires_pin = None;
    let mut credential_id = None;
    let mut salt = None;
    let mut ciphertext = None;

    for field in rest.split_whitespace() {
        let (key, value) = field
            .split_once('=')
            .ok_or_else(|| bad(&format!("expected key=value, got {field:?}")))?;
        match key {
            "name" => label = Some(value.to_string()),
            "rp" => rp_id = Some(value.to_string()),
            "pin" => requires_pin = Some(value == "yes"),
            "cred" => credential_id = Some(decode_slot(value)?),
            "salt" => salt = Some(decode_slot(value)?),
            "data" => ciphertext = Some(decode_slot(value)?),
            // Unknown fields are tolerated for forward compatibility.
            _ => {}
        }
    }

    let label = label.ok_or_else(|| bad("no name"))?;
    if !is_valid_label(&label) {
        return Err(bad(&format!("invalid name {label:?}")));
    }
    let salt = salt.ok_or_else(|| bad("no salt"))?;
    let salt: [u8; HMAC_SECRET_LEN] = salt.try_into().map_err(|_| bad("salt is not 32 bytes"))?;
    let credential_id = credential_id.ok_or_else(|| bad("no credential id"))?;
    if credential_id.is_empty() {
        return Err(bad("empty credential id"));
    }

    Ok(Fido2Slot {
        label,
        rp_id: rp_id.ok_or_else(|| bad("no relying-party id"))?,
        credential_id,
        salt,
        requires_pin: requires_pin.unwrap_or(false),
        ciphertext: ciphertext.ok_or_else(|| bad("no ciphertext"))?,
    })
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
    open_slot(ciphertext, secret, "wrong passphrase or corrupt slot")
}

fn open_slot(ciphertext: &[u8], secret: &SecretString, what: &str) -> Result<String> {
    let identity = age::scrypt::Identity::new(secret.clone());
    let plaintext = age::decrypt(&identity, ciphertext)
        .map_err(|e| Error::AuthFailed(format!("{what}: {e}")))?;
    String::from_utf8(plaintext)
        .map_err(|e| Error::AuthFailed(format!("decrypted slot is not valid UTF-8: {e}")))
}

/// A 256-bit key rendered as the passphrase age's scrypt recipient expects.
fn key_as_secret(key: &[u8; HMAC_SECRET_LEN]) -> SecretString {
    SecretString::from(b64().encode(key))
}

/// Seal `identity_secret` under a full-entropy 256-bit key — the HMAC output
/// a security key releases after a touch. Unlike [`encrypt_slot`], this uses
/// a deliberately cheap KDF: see [`HIGH_ENTROPY_LOG_N`].
pub fn encrypt_slot_with_key(
    identity_secret: &str,
    key: &[u8; HMAC_SECRET_LEN],
) -> Result<Vec<u8>> {
    let mut recipient = age::scrypt::Recipient::new(key_as_secret(key));
    recipient.set_work_factor(HIGH_ENTROPY_LOG_N);
    age::encrypt(&recipient, identity_secret.as_bytes())
        .map_err(|e| Error::AuthFailed(format!("could not seal security-key slot: {e}")))
}

/// Open a slot sealed by [`encrypt_slot_with_key`]. A key from the wrong
/// authenticator (or the right one given the wrong salt) surfaces as
/// [`Error::AuthFailed`].
pub fn decrypt_slot_with_key(ciphertext: &[u8], key: &[u8; HMAC_SECRET_LEN]) -> Result<String> {
    open_slot(
        ciphertext,
        &key_as_secret(key),
        "the security key did not produce the right secret",
    )
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

    fn fido2(label: &str) -> Fido2Slot {
        Fido2Slot {
            label: label.to_string(),
            rp_id: "nopass".to_string(),
            credential_id: vec![1, 2, 3, 4],
            salt: [7u8; HMAC_SECRET_LEN],
            requires_pin: false,
            ciphertext: b"sealed".to_vec(),
        }
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
        };
        let text = locked.serialize();

        let reparsed = LockedIdentity::parse(&text).unwrap();
        let recovered =
            decrypt_slot(reparsed.passphrase_slot.as_ref().unwrap(), &pass("master")).unwrap();
        assert_eq!(&recovered, secret);
        // And it parses back into a working identity.
        assert!(recovered.parse::<age::x25519::Identity>().is_ok());
    }

    // ---- FIDO2 security-key slots ----

    #[test]
    fn fido2_slot_roundtrips_through_the_file_format() {
        let locked = LockedIdentity {
            passphrase_slot: Some(b"pw".to_vec()),
            fido2_slots: vec![Fido2Slot {
                requires_pin: true,
                ciphertext: vec![0u8, 255, 128, 7],
                ..fido2("yubikey")
            }],
            ..Default::default()
        };
        let parsed = LockedIdentity::parse(&locked.serialize()).unwrap();
        assert_eq!(parsed.fido2_slots, locked.fido2_slots);
        assert!(parsed.has_fido2());
        assert_eq!(parsed.slot_count(), 2);
    }

    #[test]
    fn several_security_keys_keep_their_order_and_identity() {
        let locked = LockedIdentity {
            passphrase_slot: Some(b"pw".to_vec()),
            fido2_slots: vec![
                fido2("primary"),
                Fido2Slot {
                    credential_id: vec![9, 9],
                    salt: [3u8; HMAC_SECRET_LEN],
                    ..fido2("backup")
                },
            ],
            ..Default::default()
        };
        let parsed = LockedIdentity::parse(&locked.serialize()).unwrap();
        assert_eq!(
            parsed
                .fido2_slots
                .iter()
                .map(|s| s.label.as_str())
                .collect::<Vec<_>>(),
            ["primary", "backup"]
        );
        assert_eq!(parsed.fido2_slot("backup").unwrap().credential_id, [9, 9]);
        assert!(parsed.fido2_slot("nope").is_none());
    }

    #[test]
    fn a_security_key_alone_is_enough_to_be_a_valid_lock() {
        // No passphrase slot: unusual, but the file must still parse rather
        // than being mistaken for an empty lock.
        let locked = LockedIdentity {
            fido2_slots: vec![fido2("only")],
            ..Default::default()
        };
        let parsed = LockedIdentity::parse(&locked.serialize()).unwrap();
        assert!(!parsed.has_passphrase());
        assert_eq!(parsed.slot_count(), 1);
    }

    #[test]
    fn passphrase_only_files_from_older_versions_still_parse() {
        // Byte-for-byte what nopass wrote before security keys existed.
        let text = format!(
            "{LOCK_HEADER}\n# Encrypted identity — unlock with passkey/passphrase.\n\
             slot-passphrase: {}\n",
            b64().encode(b"legacy")
        );
        let parsed = LockedIdentity::parse(&text).unwrap();
        assert_eq!(parsed.passphrase_slot.as_deref(), Some(&b"legacy"[..]));
        assert!(!parsed.has_fido2());
    }

    #[test]
    fn unknown_slot_types_are_ignored_rather_than_fatal() {
        // Forward compatibility: a file written by a newer nopass must still
        // hand this build its passphrase slot.
        let text = format!(
            "{LOCK_HEADER}\nslot-passphrase: {}\nslot-quantum: whatever\n",
            b64().encode(b"pw")
        );
        let parsed = LockedIdentity::parse(&text).unwrap();
        assert!(parsed.has_passphrase());
        assert!(!parsed.has_fido2());
    }

    #[test]
    fn malformed_security_key_slots_are_rejected() {
        let cases = [
            // no fields at all
            "slot-fido2:",
            // missing the ciphertext
            "slot-fido2: name=k rp=nopass cred=AQID salt=Bw==",
            // salt of the wrong length
            "slot-fido2: name=k rp=nopass cred=AQID salt=Bw== data=AQ==",
            // a name that could not have come from us
            "slot-fido2: name=has space rp=nopass cred=AQID salt=Bw== data=AQ==",
            // not key=value
            "slot-fido2: garbage",
        ];
        for case in cases {
            let text = format!("{LOCK_HEADER}\n{case}\n");
            assert!(
                LockedIdentity::parse(&text).is_err(),
                "should have rejected: {case}"
            );
        }
    }

    #[test]
    fn labels_must_be_short_and_shell_safe() {
        assert!(is_valid_label("yubikey-5.nfc_2"));
        assert!(!is_valid_label(""));
        assert!(!is_valid_label("has space"));
        assert!(!is_valid_label("emoji🔑"));
        assert!(!is_valid_label(&"x".repeat(33)));
    }

    #[test]
    fn key_sealed_slot_roundtrips() {
        let secret = "AGE-SECRET-KEY-1EXAMPLE";
        let key = [42u8; HMAC_SECRET_LEN];
        let ct = encrypt_slot_with_key(secret, &key).unwrap();
        assert_eq!(decrypt_slot_with_key(&ct, &key).unwrap(), secret);
    }

    #[test]
    fn key_sealed_slot_rejects_another_authenticators_output() {
        let ct = encrypt_slot_with_key("AGE-SECRET-KEY-1EXAMPLE", &[1u8; HMAC_SECRET_LEN]).unwrap();
        assert!(matches!(
            decrypt_slot_with_key(&ct, &[2u8; HMAC_SECRET_LEN]),
            Err(Error::AuthFailed(_))
        ));
        // Even a near-miss key must not open it.
        let mut almost = [1u8; HMAC_SECRET_LEN];
        almost[31] = 2;
        assert!(decrypt_slot_with_key(&ct, &almost).is_err());
    }

    #[test]
    fn full_lock_unlock_via_security_key_slot() {
        let identity = age::x25519::Identity::generate();
        use age::secrecy::ExposeSecret;
        let secret = identity.to_string();
        let secret = secret.expose_secret();

        let key = [0xABu8; HMAC_SECRET_LEN];
        let locked = LockedIdentity {
            passphrase_slot: Some(encrypt_slot(secret, &pass("master")).unwrap()),
            fido2_slots: vec![Fido2Slot {
                ciphertext: encrypt_slot_with_key(secret, &key).unwrap(),
                ..fido2("yubikey")
            }],
            ..Default::default()
        };

        // Both slots recover the very same identity.
        let parsed = LockedIdentity::parse(&locked.serialize()).unwrap();
        let via_key = decrypt_slot_with_key(&parsed.fido2_slots[0].ciphertext, &key).unwrap();
        let via_pass =
            decrypt_slot(parsed.passphrase_slot.as_ref().unwrap(), &pass("master")).unwrap();
        assert_eq!(via_key, via_pass);
        assert_eq!(&via_key, secret);
        assert!(via_key.parse::<age::x25519::Identity>().is_ok());
    }
}
