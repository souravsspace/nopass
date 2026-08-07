//! First-run setup: deciding where this machine's private key lives and
//! locking it behind a master passphrase before it is ever written.
//!
//! A password manager that silently drops an unprotected key somewhere in
//! `~/.config` teaches the user nothing about what they now have to back up.
//! So the first time nopass needs a keypair it says where the key is going,
//! offers to put it elsewhere, and asks for a passphrase — after which every
//! read of a password requires it.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use nopass_core::config;
use nopass_core::crypto;
use nopass_core::lock::SecretString;

use crate::prompt_hidden;

/// Shown whenever setup needs an answer and stdin has none.
const UNATTENDED_HINT: &str = "\nError: first-run setup needs a terminal to ask where your \
     private key should live\nand what passphrase protects it. For unattended setup, answer \
     with flags:\n  nopass init --identity <path> --no-passphrase";

/// Ask for a visible answer, turning "nothing on stdin" into the hint above.
fn ask(prompt: &str) -> Result<String> {
    crate::prompt_line(prompt).map_err(|_| anyhow::anyhow!("{UNATTENDED_HINT}"))
}

/// Ask for an answer with echo off, same treatment.
fn ask_hidden(prompt: &str) -> Result<String> {
    prompt_hidden(prompt).map_err(|_| anyhow::anyhow!("{UNATTENDED_HINT}"))
}

/// Filename used when the user names a directory rather than a file.
const IDENTITY_FILENAME: &str = "identity.txt";

/// Folder nopass makes for itself inside a directory the user names.
const IDENTITY_DIRNAME: &str = "nopass";

/// Where the key goes inside a directory the user pointed at: its own
/// `nopass/` folder, the same shape as the default `~/.config/nopass`. Being
/// handed `~/Desktop` should not scatter an `identity.txt` across the desktop.
fn identity_in_dir(dir: &Path) -> PathBuf {
    if dir.file_name() == Some(IDENTITY_DIRNAME.as_ref()) {
        return dir.join(IDENTITY_FILENAME);
    }
    dir.join(IDENTITY_DIRNAME).join(IDENTITY_FILENAME)
}

/// Where a new keypair would be written, without asking anything. Used to
/// report "an identity already exists" before setup starts talking.
pub fn target_path(explicit: Option<&str>) -> Result<PathBuf> {
    match explicit {
        Some(path) => resolve(path),
        None => Ok(crypto::default_identity_file()),
    }
}

/// Create this machine's keypair. Returns the path it was written to and the
/// public recipient string.
pub fn create_identity(explicit: Option<&str>, no_passphrase: bool) -> Result<(PathBuf, String)> {
    let (path, remember) = choose_location(explicit, no_passphrase)?;
    let passphrase = choose_passphrase(no_passphrase)?;

    let public = match &passphrase {
        Some(passphrase) => crypto::generate_identity_locked(&path, passphrase)?,
        None => crypto::generate_identity(&path)?,
    };

    // Only record a location the user actually chose; the default path and
    // NOPASS_IDENTITY both resolve on their own.
    if remember {
        let config_file = config::default_config_file();
        config::set_identity_path(&config_file, &path).with_context(|| {
            format!(
                "could not record the key location in {}",
                config_file.display()
            )
        })?;
    }

    report(&path, &public, passphrase.is_some(), remember);
    Ok((path, public))
}

/// A path the user typed: `~` expanded, and a directory turned into the key
/// file inside it.
///
/// `~/Vaults` means a directory even before it exists — only something that
/// looks like a filename (`work.txt`) is taken as one, since that is the
/// only reading that makes `--identity ~/Vaults` do what it plainly says.
fn resolve(input: &str) -> Result<PathBuf> {
    let path = config::expand_tilde(input);
    if path.as_os_str().is_empty() {
        bail!("Error: no path given for the private key.");
    }
    let names_a_file = path.extension().is_some() && !path.is_dir();
    if names_a_file && !input.ends_with('/') {
        return Ok(path);
    }
    Ok(identity_in_dir(&path))
}

/// Decide where the key goes, and whether that choice needs remembering.
fn choose_location(explicit: Option<&str>, unattended: bool) -> Result<(PathBuf, bool)> {
    if let Some(path) = explicit {
        return Ok((resolve(path)?, true));
    }

    // Nothing to ask when the location is already settled: an environment
    // variable, a previous run's config file, a key that already exists
    // (the `keygen --force` case), or `--no-passphrase`, which is how a
    // script says it has nobody to answer questions.
    let default = crypto::default_identity_file();
    if unattended
        || std::env::var_os("NOPASS_IDENTITY").is_some()
        || config::read_identity_path(&config::default_config_file()).is_some()
        || default.exists()
    {
        return Ok((default, false));
    }

    println!("Setting up nopass.\n");
    println!("Everything in your store is encrypted to one private key. That file is");
    println!("yours to keep and to back up — without it, no password can be recovered.\n");
    println!("Where should the private key live?");
    println!("  [1] {}   (default)", default.display());
    println!("  [2] a directory you choose (nopass makes a nopass/ folder in it)");

    match ask("Choice [1]: ")?.as_str() {
        "" | "1" => Ok((default, false)),
        "2" => {
            let dir = ask("Directory: ")?;
            if dir.is_empty() {
                bail!("Error: no directory given for the private key.");
            }
            let path = identity_in_dir(&config::expand_tilde(&dir));
            let parent = path.parent().expect("the key always sits in a folder");
            std::fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
            Ok((path, true))
        }
        other => bail!("Error: {other:?} is not one of the choices (1 or 2)."),
    }
}

/// Ask for the master passphrase, twice. `None` means the user opted out and
/// the key will sit on disk unprotected.
fn choose_passphrase(no_passphrase: bool) -> Result<Option<SecretString>> {
    if no_passphrase {
        eprintln!(
            "Warning: creating an unprotected private key. Anyone who can read the\n\
             file can read every password. Run \"nopass passkey enroll\" to lock it."
        );
        return Ok(None);
    }

    println!("\nChoose a master passphrase. nopass asks for it every time it reads");
    println!("a password, and it is the only thing protecting the key file.\n");

    let passphrase = ask_hidden("Master passphrase: ")?;
    if passphrase.is_empty() {
        bail!("Error: passphrase must not be empty.");
    }
    let again = ask_hidden("Retype master passphrase: ")?;
    if passphrase != again {
        bail!("Error: the entered passphrases do not match.");
    }
    Ok(Some(SecretString::from(passphrase)))
}

fn report(path: &Path, public: &str, locked: bool, remembered: bool) {
    let state = if locked {
        "locked with your passphrase"
    } else {
        "unprotected"
    };
    println!("\nPrivate key: {} ({state}, mode 0600)", path.display());
    println!("Public key:  {public}");
    if remembered {
        println!("Remembered:  {}", config::default_config_file().display());
    }
    println!(
        "\nBack up the private key file. If you lose it{}, every",
        if locked {
            " or forget the passphrase"
        } else {
            ""
        }
    );
    println!("password in your store becomes unreadable.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_gets_its_own_nopass_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_str().unwrap();
        assert_eq!(
            resolve(dir).unwrap(),
            tmp.path().join("nopass/identity.txt"),
            "pointing at a directory should not scatter identity.txt into it"
        );
        // A trailing slash means "directory" even before it exists.
        assert_eq!(
            resolve("/nowhere/Desktop/").unwrap(),
            PathBuf::from("/nowhere/Desktop/nopass/identity.txt")
        );
    }

    #[test]
    fn a_directory_already_called_nopass_is_not_nested_twice() {
        assert_eq!(
            resolve("/keys/nopass/").unwrap(),
            PathBuf::from("/keys/nopass/identity.txt")
        );
    }

    #[test]
    fn a_filename_is_taken_literally() {
        assert_eq!(
            resolve("/keys/work-key.txt").unwrap(),
            PathBuf::from("/keys/work-key.txt")
        );
    }

    #[test]
    fn a_path_with_no_extension_is_a_directory_even_before_it_exists() {
        assert_eq!(
            resolve("/Users/me/Vaults").unwrap(),
            PathBuf::from("/Users/me/Vaults/nopass/identity.txt"),
            "--identity ~/Vaults should not create a file called Vaults"
        );
    }

    #[test]
    fn an_existing_directory_wins_over_its_extension() {
        let tmp = tempfile::tempdir().unwrap();
        let dotted = tmp.path().join("my.keys");
        std::fs::create_dir_all(&dotted).unwrap();
        assert_eq!(
            resolve(dotted.to_str().unwrap()).unwrap(),
            dotted.join("nopass/identity.txt")
        );
    }

    #[test]
    fn an_empty_path_is_rejected() {
        assert!(resolve("").is_err());
        assert!(resolve("   ").is_err());
    }
}
