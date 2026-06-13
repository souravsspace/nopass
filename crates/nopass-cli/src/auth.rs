//! Authentication for locked identities. The passphrase slot is the real,
//! cross-platform cryptographic lock; on macOS an optional Secure Enclave
//! (Touch ID) slot can be layered on top (see [`crate::touchid`], behind the
//! `touchid` feature). Either slot recovers the same identity.

use nopass_core::lock::{decrypt_slot, LockedIdentity, SecretString, Unlocker};
use nopass_core::{Error as CoreError, Result as CoreResult};

/// Front-end unlocker: tries Touch ID first when present, otherwise prompts
/// for the passphrase.
pub struct CliUnlocker;

impl Unlocker for CliUnlocker {
    fn unlock(&self, locked: &LockedIdentity) -> CoreResult<String> {
        #[cfg(all(target_os = "macos", feature = "touchid"))]
        if let Some(slot) = &locked.keychain_slot {
            match crate::touchid::unlock_slot(slot) {
                Ok(secret) => return Ok(secret),
                Err(e) => {
                    eprintln!("Touch ID unlock failed ({e}); falling back to passphrase.");
                }
            }
        }

        let slot = locked.passphrase_slot.as_ref().ok_or_else(|| {
            CoreError::AuthUnavailable(
                "no passphrase slot is present and Touch ID is unavailable".into(),
            )
        })?;
        let passphrase = prompt_passphrase("Enter your nopass passphrase: ")?;
        decrypt_slot(slot, &SecretString::from(passphrase))
    }
}

/// Prompt for a passphrase with echo disabled. Maps I/O failures into the
/// core error type so it can satisfy [`Unlocker`].
pub fn prompt_passphrase(prompt: &str) -> CoreResult<String> {
    crate::prompt_hidden(prompt).map_err(|e| CoreError::AuthFailed(e.to_string()))
}
