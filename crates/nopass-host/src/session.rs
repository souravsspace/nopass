//! Unlock state, borrowed from the CLI rather than reinvented.
//!
//! The host does not keep a cache of its own: it reads and writes the same
//! `nopass agent` the terminal uses, so `nopass lock` locks the browser too
//! and there is only one place to audit (ADR-0005).

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use nopass_core::lock::{decrypt_slot, LockedIdentity, SecretString, Unlocker};
use nopass_core::{agent, config, crypto, Error as CoreError, Result as CoreResult};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::proto::LockState;

/// What the agent files a secret under: a digest of the identity exactly as
/// it sits on disk, matching what the CLI computes. Relocking the identity
/// rewrites that file, so the old entry stops matching rather than quietly
/// outliving the passphrase that created it.
pub fn cache_key(locked: &LockedIdentity) -> String {
    Sha256::digest(locked.serialize().as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// An [`Unlocker`] that will consult the agent and nothing else.
///
/// The CLI's unlocker prompts when the cache is cold. This one cannot: it is
/// running as a child of the browser with no terminal attached. A cold cache
/// is reported as locked, and the extension asks.
pub struct AgentOnly;

impl Unlocker for AgentOnly {
    fn unlock(&self, locked: &LockedIdentity) -> CoreResult<String> {
        agent::get(&cache_key(locked)).ok_or(CoreError::Locked)
    }
}

pub struct Session {
    identity_file: PathBuf,
    /// Seconds an unlocked identity stays in the agent. Read from the same
    /// config the CLI reads, so the two never disagree. An explicit zero
    /// means the extension asks every time; unset, the shipped default
    /// (`config::DEFAULT_CACHE_TTL`) applies.
    ttl: u64,
}

impl Default for Session {
    fn default() -> Self {
        Self::new(crypto::default_identity_file())
    }
}

impl Session {
    pub fn new(identity_file: PathBuf) -> Self {
        let ttl = config::read_cache_ttl(&config::default_config_file()).unwrap_or(config::DEFAULT_CACHE_TTL);
        Self { identity_file, ttl }
    }

    /// The identity file, if it is locked. A plaintext identity has nothing
    /// to unlock, so it reads as `None` rather than as an error.
    fn locked(&self) -> Option<LockedIdentity> {
        let text = std::fs::read_to_string(&self.identity_file).ok()?;
        LockedIdentity::is_locked_file(&text)
            .then(|| LockedIdentity::parse(&text).ok())
            .flatten()
    }

    /// Whether the store can be read right now, and for how much longer.
    pub fn state(&self) -> (LockState, u64) {
        let Some(locked) = self.locked() else {
            return (LockState::Unlocked, 0);
        };

        // Ask for the secret rather than for a count: the agent may be
        // holding somebody else's identity, which does not unlock this one.
        if agent::get(&cache_key(&locked)).is_some() {
            let remaining = agent::status().map_or(0, |(_, seconds)| seconds);
            (LockState::Unlocked, remaining)
        } else {
            (LockState::Locked, 0)
        }
    }

    /// Open the passphrase slot and hand the result to the agent. Returns how
    /// long it will be held for.
    ///
    /// The agent is the only place this unlock can live — the host keeps no
    /// cache and exits with the port — so a lease that is not stored is a
    /// lease that does not exist, and every read after it would be told the
    /// store is locked. Both ways of failing to store one are reported here
    /// rather than answered with an unlocked state that undoes itself on the
    /// next request.
    pub fn unlock(&self, passphrase: &str) -> Result<u64> {
        let Some(locked) = self.locked() else {
            // Nothing is locked, so the passphrase was not needed.
            return Ok(0);
        };

        if self.ttl == 0 {
            bail!(
                "the passphrase cache is off (`cache-ttl = 0`), so an unlock \
                 could not be held: set `cache-ttl` to a number of seconds in {}",
                config::default_config_file().display()
            );
        }

        let slot = locked
            .passphrase_slot
            .as_ref()
            .context("this identity has no passphrase slot to open")?;

        // Said plainly, because the popup shows this to the person typing:
        // the cause is kept on the chain, but "Decryption failed" is not an
        // answer to "did I get my passphrase wrong?".
        let secret = Zeroizing::new(
            decrypt_slot(slot, &SecretString::from(passphrase.to_owned()))
                .context("that passphrase did not open the store")?,
        );
        let key = cache_key(&locked);
        agent::put(&key, &secret, self.ttl);

        // `put` is best effort by design — the CLI can simply ask again. The
        // browser cannot, so read it back before promising a lease.
        if agent::get(&key).map(Zeroizing::new).is_none() {
            bail!(
                "the passphrase was right, but the agent did not keep it: \
                 check that a current `nopass` sits beside the host or on \
                 PATH, and can run `nopass agent serve`"
            );
        }
        Ok(self.ttl)
    }

    /// Forget everything the agent is holding, for the terminal as well.
    pub fn lock(&self) {
        agent::clear();
    }
}
