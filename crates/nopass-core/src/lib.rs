//! nopass-core: the engine behind nopass, a fast, self-contained password
//! manager. Entries are individually encrypted files in a directory tree,
//! with per-directory recipient files and automatic git commits.

pub mod crypto;
pub mod error;
pub mod generate;
pub mod git;
pub mod paths;
pub mod store;

pub use crypto::{Crypto, GpgCrypto, NativeCrypto, PlainCrypto};
pub use error::{Error, Result};
pub use store::{GrepHit, Store};

/// Store location: NOPASS_DIR or ~/.nopass.
pub fn default_store_dir() -> std::path::PathBuf {
    std::env::var_os("NOPASS_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".nopass")
        })
}

/// Crypto backend selection via NOPASS_BACKEND: "native" (default, built-in
/// age encryption), "gpg" (shell out to gpg), or "plain" (tests only).
pub fn default_crypto() -> Box<dyn Crypto> {
    match std::env::var("NOPASS_BACKEND").as_deref() {
        Ok("plain") => Box::new(PlainCrypto),
        Ok("gpg") => Box::new(GpgCrypto::new()),
        _ => Box::new(NativeCrypto::new()),
    }
}
