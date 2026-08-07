//! The pointer file that remembers where the user asked nopass to keep their
//! private key.
//!
//! Everything in a store is encrypted to one keypair, and the secret half of
//! it is a single file the user is expected to back up. Some people want it
//! on an encrypted volume, a YubiKey-backed directory, or a synced folder
//! rather than in `~/.config`. Rather than making them export an environment
//! variable in every shell, first-run setup records the choice here and every
//! later run reads it back.

use std::path::{Path, PathBuf};

use crate::error::Result;

/// Overrides the location of the config file itself.
pub const CONFIG_ENV: &str = "NOPASS_CONFIG";

/// The only key we currently store.
const IDENTITY_KEY: &str = "identity";

/// The user's home directory, or an empty path if `HOME` is unset.
pub fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

/// Config location: `NOPASS_CONFIG`, else `~/.config/nopass/config`.
pub fn default_config_file() -> PathBuf {
    if let Some(path) = std::env::var_os(CONFIG_ENV) {
        return PathBuf::from(path);
    }
    home().join(".config/nopass/config")
}

/// Where a fresh install puts the private key when the user doesn't choose.
pub fn default_identity_location() -> PathBuf {
    home().join(".config/nopass/identity.txt")
}

/// Expand a leading `~` so users can type paths the way they say them.
pub fn expand_tilde(input: &str) -> PathBuf {
    let input = input.trim();
    match input.strip_prefix('~') {
        Some("") => home(),
        Some(rest) => {
            // Only `~/...` — `~someone-else/...` is not ours to resolve.
            match rest.strip_prefix('/') {
                Some(rest) => home().join(rest),
                None => PathBuf::from(input),
            }
        }
        None => PathBuf::from(input),
    }
}

/// Pull the identity path out of a config file's text.
pub fn identity_path_in(text: &str) -> Option<PathBuf> {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim() == IDENTITY_KEY)
        .map(|(_, value)| expand_tilde(value))
        .filter(|path| !path.as_os_str().is_empty())
}

/// Read the recorded identity path, if a config file exists and names one.
pub fn read_identity_path(config_file: &Path) -> Option<PathBuf> {
    identity_path_in(&std::fs::read_to_string(config_file).ok()?)
}

/// Record `identity` as the private key location, creating the config file.
pub fn set_identity_path(config_file: &Path, identity: &Path) -> Result<()> {
    if let Some(parent) = config_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        config_file,
        format!(
            "# nopass configuration — written by \"nopass init\"\n\
             # Path to the private key everything in the store is encrypted to.\n\
             {IDENTITY_KEY} = {}\n",
            identity.display()
        ),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_identity_key() {
        assert_eq!(
            identity_path_in("identity = /keys/id.txt\n"),
            Some(PathBuf::from("/keys/id.txt"))
        );
    }

    #[test]
    fn ignores_comments_blanks_and_other_keys() {
        let text = "# a comment\n\nsomething = else\nidentity = /k/id.txt # trailing\n";
        assert_eq!(
            identity_path_in(text),
            Some(PathBuf::from("/k/id.txt")),
            "comments and unrelated keys must not confuse the parser"
        );
        assert_eq!(identity_path_in("# identity = /k/id.txt\n"), None);
        assert_eq!(identity_path_in(""), None);
        assert_eq!(identity_path_in("identity =\n"), None);
    }

    #[test]
    fn the_first_identity_line_wins() {
        assert_eq!(
            identity_path_in("identity = /first\nidentity = /second\n"),
            Some(PathBuf::from("/first"))
        );
    }

    #[test]
    fn tilde_expands_only_for_the_current_user() {
        let home = home();
        assert_eq!(expand_tilde("~/keys/id.txt"), home.join("keys/id.txt"));
        assert_eq!(expand_tilde("~"), home);
        assert_eq!(expand_tilde("~other/keys"), PathBuf::from("~other/keys"));
        assert_eq!(expand_tilde("/abs/path"), PathBuf::from("/abs/path"));
        assert_eq!(expand_tilde("  /spaced  "), PathBuf::from("/spaced"));
    }

    #[test]
    fn written_config_reads_back() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("nested/config");
        let identity = tmp.path().join("keys/id.txt");

        set_identity_path(&config, &identity).unwrap();
        assert_eq!(read_identity_path(&config).unwrap(), identity);

        // Rewriting points somewhere else rather than appending a second key.
        let moved = tmp.path().join("elsewhere/id.txt");
        set_identity_path(&config, &moved).unwrap();
        assert_eq!(read_identity_path(&config).unwrap(), moved);
    }

    #[test]
    fn a_missing_config_is_not_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(read_identity_path(&tmp.path().join("nope")).is_none());
    }
}
