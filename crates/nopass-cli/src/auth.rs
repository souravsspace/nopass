//! Authentication for locked identities. The passphrase slot is the real,
//! cross-platform cryptographic lock; a FIDO2 security key (see
//! [`crate::fido2`]) and, on macOS, a Secure Enclave / Touch ID slot (see
//! [`crate::touchid`]) can be layered on top. Every slot recovers the same
//! identity, so unlocking tries them cheapest-first and falls through to the
//! passphrase whenever a factor is missing or refuses.

use nopass_core::lock::{
    decrypt_slot, decrypt_slot_with_key, Fido2Slot, LockedIdentity, SecretString, Unlocker,
};
use nopass_core::{Error as CoreError, Result as CoreResult};

use crate::fido2;

/// Set `NOPASS_UNLOCK=passphrase` to skip the hardware factors — useful when
/// a key is plugged in but you would rather just type.
const FORCE_ENV: &str = "NOPASS_UNLOCK";

/// Front-end unlocker: Touch ID, then any enrolled security key, then the
/// passphrase.
pub struct CliUnlocker;

impl Unlocker for CliUnlocker {
    fn unlock(&self, locked: &LockedIdentity) -> CoreResult<String> {
        let passphrase_only = std::env::var(FORCE_ENV).as_deref() == Ok("passphrase");

        #[cfg(target_os = "macos")]
        if !passphrase_only {
            if let Some(slot) = &locked.keychain_slot {
                match crate::touchid::unlock_slot(slot) {
                    Ok(secret) => return Ok(secret),
                    Err(e) => {
                        eprintln!("Touch ID unlock failed ({e}); trying the next factor.");
                    }
                }
            }
        }

        if !passphrase_only {
            if let Some(secret) = try_security_keys(locked) {
                return Ok(secret);
            }
        }

        let slot = locked.passphrase_slot.as_ref().ok_or_else(|| {
            CoreError::AuthUnavailable(
                "no passphrase slot is present and no enrolled security key could be used".into(),
            )
        })?;
        let passphrase = prompt_passphrase("Enter your nopass passphrase: ")?;
        decrypt_slot(slot, &SecretString::from(passphrase))
    }
}

/// Try every enrolled security key slot. Returns `None` — rather than an
/// error — whenever the passphrase is still worth asking for, so a forgotten
/// key at the office never locks anyone out of their own store.
fn try_security_keys(locked: &LockedIdentity) -> Option<String> {
    if !locked.has_fido2() {
        return None;
    }
    let Some(authenticator) = fido2::detect() else {
        eprintln!(
            "Note: a security key is enrolled but unusable here ({}).",
            fido2::unavailable_reason()
        );
        return None;
    };

    for slot in &locked.fido2_slots {
        match unlock_with(authenticator.as_ref(), slot) {
            Ok(secret) => return Some(secret),
            Err(e) => eprintln!("Security key \"{}\" did not unlock ({e}).", slot.label),
        }
    }
    None
}

fn unlock_with(authenticator: &dyn fido2::SecurityKey, slot: &Fido2Slot) -> CoreResult<String> {
    let pin = if slot.requires_pin {
        Some(prompt_passphrase(&format!(
            "PIN for security key \"{}\": ",
            slot.label
        ))?)
    } else {
        None
    };
    eprintln!("Touch your security key \"{}\"…", slot.label);
    let secret =
        authenticator.derive(&slot.rp_id, &slot.credential_id, &slot.salt, pin.as_deref())?;
    decrypt_slot_with_key(&slot.ciphertext, &secret)
}

/// Prompt for a passphrase with echo disabled. Maps I/O failures into the
/// core error type so it can satisfy [`Unlocker`].
pub fn prompt_passphrase(prompt: &str) -> CoreResult<String> {
    crate::prompt_hidden(prompt).map_err(|e| CoreError::AuthFailed(e.to_string()))
}
