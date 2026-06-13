use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Error: refusing a path that escapes the store.")]
    SneakyPath,

    #[error("Error: {0} is not in the store.")]
    NotInStore(String),

    #[error("Error: store is empty. Try \"nopass init\".")]
    EmptyStore,

    #[error("Error: no recipients configured. Run \"nopass init\" first.")]
    NoRecipients,

    #[error("Error: {0} exists but is not a directory.")]
    NotADirectory(PathBuf),

    #[error("Error: password length must be greater than zero.")]
    ZeroLength,

    #[error("encryption failed: {0}")]
    Encrypt(String),

    #[error("decryption failed: {0}")]
    Decrypt(String),

    #[error("no identity found at {0}. Run \"nopass keygen\" first.")]
    NoIdentity(PathBuf),

    #[error("Error: the identity is locked but no authentication is available to unlock it.")]
    Locked,

    #[error("authentication failed: {0}")]
    AuthFailed(String),

    #[error("authentication is unavailable on this system: {0}")]
    AuthUnavailable(String),

    #[error("Error: the locked identity file at {0} is malformed.")]
    MalformedLock(PathBuf),

    #[error("git: {0}")]
    Git(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
