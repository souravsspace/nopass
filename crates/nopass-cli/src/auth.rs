//! Authentication for locked identities. The passphrase slot is the real,
//! cross-platform cryptographic lock; a FIDO2 security key (see
//! [`crate::fido2`]) can be layered on top. Every slot recovers the same
//! identity, so unlocking tries the key first and falls through to the
//! passphrase whenever it is missing or refuses.

use sha2::{Digest, Sha256};

use nopass_core::config;
use nopass_core::lock::{
    decrypt_slot, decrypt_slot_with_key, Fido2Slot, LockedIdentity, SecretString, Unlocker,
};
use nopass_core::{Error as CoreError, Result as CoreResult};

use crate::agent;
use crate::fido2;

/// Set `NOPASS_UNLOCK=passphrase` to skip the hardware factor — useful when
/// a key is plugged in but you would rather just type.
const FORCE_ENV: &str = "NOPASS_UNLOCK";

/// Seconds to keep an unlocked identity in the agent, overriding the config
/// file. Zero, the default, means no caching.
const TTL_ENV: &str = "NOPASS_CACHE_TTL";

/// Front-end unlocker: any enrolled security key, then the passphrase.
pub struct CliUnlocker;

impl Unlocker for CliUnlocker {
    fn unlock(&self, locked: &LockedIdentity) -> CoreResult<String> {
        let passphrase_only = std::env::var(FORCE_ENV).as_deref() == Ok("passphrase");

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

/// Whether a command may be let in by a passphrase someone typed earlier.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AuthPolicy {
    /// Reading: a cached passphrase counts.
    Cached,
    /// Changing the store: the owner types it, every time.
    Fresh,
}

/// [`CliUnlocker`] with the passphrase cache in front of it. What it gets
/// from the user is offered to the agent either way, so a write warms the
/// cache for the reads that follow.
pub struct CachingUnlocker {
    policy: AuthPolicy,
    ttl: u64,
}

impl CachingUnlocker {
    pub fn new(policy: AuthPolicy) -> Self {
        Self {
            policy,
            ttl: cache_ttl(),
        }
    }
}

impl Unlocker for CachingUnlocker {
    fn unlock(&self, locked: &LockedIdentity) -> CoreResult<String> {
        if self.ttl == 0 {
            return CliUnlocker.unlock(locked);
        }
        let key = cache_key(locked);
        if self.policy == AuthPolicy::Cached {
            if let Some(secret) = agent::get(&key) {
                return Ok(secret);
            }
        }
        let secret = CliUnlocker.unlock(locked)?;
        agent::put(&key, &secret, self.ttl);
        Ok(secret)
    }
}

/// How long an unlocked identity may be cached: the environment wins over
/// the config file, and the default is not to cache at all.
fn cache_ttl() -> u64 {
    if let Some(ttl) = std::env::var(TTL_ENV).ok().and_then(|v| v.parse().ok()) {
        return ttl;
    }
    config::read_cache_ttl(&config::default_config_file()).unwrap_or(0)
}

/// What the agent files a secret under: a digest of the identity exactly as
/// it sits on disk. Locking it again — a new passphrase, another security
/// key — rewrites that file, so the old entry stops matching instead of
/// quietly outliving the passphrase that created it.
fn cache_key(locked: &LockedIdentity) -> String {
    Sha256::digest(locked.serialize().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
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
