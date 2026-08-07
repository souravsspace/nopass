//! macOS Touch ID slot, backed by a Secure Enclave key.
//!
//! Enrollment generates a non-extractable EC key in the Secure Enclave whose
//! use is gated by biometry, then ECIES-encrypts the age identity secret to
//! that key's public half. Unlocking decrypts with the private key, which the
//! system only permits after a successful Touch ID prompt.
//!
//! Note: creating and using biometry-gated Secure Enclave keys requires the
//! binary to be code-signed with keychain entitlements and an embedded
//! provisioning profile — see `packaging/macos`. A build from source (plain
//! `cargo install`, or Homebrew building the formula) cannot do it and fails
//! at enrollment with `errSecMissingEntitlement`; the passphrase slot works
//! regardless, so this is always a skipped slot rather than a hard failure.

#![cfg(target_os = "macos")]

use security_framework::access_control::{ProtectionMode, SecAccessControl};
use security_framework::item::{
    ItemClass, ItemSearchOptions, KeyClass, Limit, Location, Reference, SearchResult,
};
use security_framework::key::{Algorithm, GenerateKeyOptions, KeyType, SecKey, Token};
use security_framework_sys::access_control::{
    kSecAccessControlBiometryAny, kSecAccessControlPrivateKeyUsage,
};

use nopass_core::{Error as CoreError, Result as CoreResult};

/// Keychain label used to find our Secure Enclave key again.
const LABEL: &str = "nopass identity (Touch ID)";
/// ECIES variant recommended by Apple for Secure Enclave EC keys.
const ALG: Algorithm = Algorithm::ECIESEncryptionCofactorX963SHA256AESGCM;
/// `errSecMissingEntitlement`: what an unsigned build gets back, and the one
/// failure worth explaining in plain words.
const ERR_SEC_MISSING_ENTITLEMENT: isize = -34018;

fn auth_failed(msg: impl std::fmt::Display) -> CoreError {
    CoreError::AuthFailed(msg.to_string())
}

fn unavailable(msg: impl std::fmt::Display) -> CoreError {
    CoreError::AuthUnavailable(msg.to_string())
}

/// Locate the enrolled Secure Enclave private key, if any. Returns `Ok(None)`
/// when nothing is enrolled (rather than erroring).
fn find_key() -> Option<SecKey> {
    let results = ItemSearchOptions::new()
        .class(ItemClass::key())
        .key_class(KeyClass::private())
        .label(LABEL)
        .load_refs(true)
        .limit(Limit::Max(1))
        .search()
        .ok()?;
    results.into_iter().find_map(|r| match r {
        SearchResult::Ref(Reference::Key(k)) => Some(k),
        _ => None,
    })
}

fn generate_key() -> CoreResult<SecKey> {
    let flags = kSecAccessControlPrivateKeyUsage | kSecAccessControlBiometryAny;
    let access = SecAccessControl::create_with_protection(
        Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
        flags,
    )
    .map_err(|e| unavailable(format!("could not build access control: {e}")))?;

    let mut opts = GenerateKeyOptions::default();
    opts.set_key_type(KeyType::ec_sec_prime_random())
        .set_size_in_bits(256)
        .set_token(Token::SecureEnclave)
        .set_label(LABEL)
        .set_location(Location::DataProtectionKeychain)
        .set_access_control(access);

    SecKey::new(&opts).map_err(|e| {
        if e.code() == ERR_SEC_MISSING_ENTITLEMENT {
            unavailable(
                "this build of nopass is not code-signed, so macOS refuses to create a \
                 Secure Enclave key. Install the signed release to use Touch ID; a \
                 passphrase or security key works with any build.",
            )
        } else {
            unavailable(format!("Secure Enclave key generation failed: {e}"))
        }
    })
}

/// Enroll a Touch ID slot: reuse or create the Secure Enclave key and
/// ECIES-encrypt `identity_secret` to its public key. Returns the slot bytes.
pub fn enroll_slot(identity_secret: &str) -> CoreResult<Vec<u8>> {
    let key = match find_key() {
        Some(k) => k,
        None => generate_key()?,
    };
    let public = key
        .public_key()
        .ok_or_else(|| auth_failed("could not derive the Secure Enclave public key"))?;
    public
        .encrypt_data(ALG, identity_secret.as_bytes())
        .map_err(|e| auth_failed(format!("ECIES encryption failed: {e}")))
}

/// Unlock a Touch ID slot, prompting Touch ID. Returns the identity secret.
pub fn unlock_slot(ciphertext: &[u8]) -> CoreResult<String> {
    let key =
        find_key().ok_or_else(|| unavailable("no Touch ID key is enrolled on this machine"))?;
    let plain = key
        .decrypt_data(ALG, ciphertext)
        .map_err(|e| auth_failed(format!("Touch ID decryption failed: {e}")))?;
    String::from_utf8(plain).map_err(|e| auth_failed(format!("decrypted data is not UTF-8: {e}")))
}

/// Remove the enrolled Secure Enclave key. Best-effort; succeeds if nothing
/// is enrolled.
pub fn remove_key() -> CoreResult<()> {
    if let Some(key) = find_key() {
        key.delete()
            .map_err(|e| auth_failed(format!("could not delete Touch ID key: {e}")))?;
    }
    Ok(())
}
