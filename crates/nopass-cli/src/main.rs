use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use nopass_core::lock::{
    encrypt_slot, encrypt_slot_with_key, is_valid_label, Fido2Slot, LockedIdentity, SecretString,
};
use nopass_core::{crypto, default_store_dir, git, Crypto, NativeCrypto, Store, Unlocker};

mod auth;
mod fido2;
mod setup;
#[cfg(all(target_os = "macos", feature = "touchid"))]
mod touchid;

/// Build the crypto backend, attaching the interactive unlocker so locked
/// (passkey/passphrase) identities prompt for authentication on use.
fn build_crypto() -> Box<dyn Crypto> {
    match std::env::var("NOPASS_BACKEND").as_deref() {
        Ok("plain") => Box::new(nopass_core::PlainCrypto),
        Ok("gpg") => Box::new(nopass_core::GpgCrypto::new()),
        _ => Box::new(NativeCrypto::new().with_unlocker(Box::new(auth::CliUnlocker))),
    }
}

#[derive(Parser)]
#[command(
    name = "nopass",
    version,
    about = "nopass: a fast, self-contained password manager",
    after_help = "Run \"nopass help\" for a tour of every command.\nhttps://github.com/souravsspace/nopass",
    args_conflicts_with_subcommands = true,
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,

    /// With no subcommand: entry to show, or subfolder to list.
    name: Option<String>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate a new encryption keypair for this machine
    Keygen {
        /// Overwrite an existing identity file
        #[arg(short, long)]
        force: bool,
        /// Where to keep the private key (a file or a directory)
        #[arg(long, value_name = "path")]
        identity: Option<String>,
        /// Leave the private key unprotected instead of asking for a passphrase
        #[arg(long)]
        no_passphrase: bool,
    },
    /// Initialize the store (generates a keypair first if you have none)
    Init {
        /// Subfolder to (re)initialize
        #[arg(short, long, default_value = "")]
        path: String,
        /// Where to keep the private key (a file or a directory)
        #[arg(long, value_name = "path")]
        identity: Option<String>,
        /// Leave the private key unprotected instead of asking for a passphrase
        #[arg(long)]
        no_passphrase: bool,
        /// Recipients (defaults to your own key from `nopass keygen`)
        recipients: Vec<String>,
    },
    /// List entries
    Ls { subfolder: Option<String> },
    /// Show an entry
    Show {
        /// Copy line N of the entry to the clipboard instead of printing
        #[arg(short, long, value_name = "line", num_args = 0..=1, default_missing_value = "1")]
        clip: Option<usize>,
        name: Option<String>,
    },
    /// List entries matching the given terms
    Find { terms: Vec<String> },
    /// Search decrypted contents for a string
    Grep { pattern: String },
    /// Insert a new entry
    Insert {
        /// Echo the password during entry
        #[arg(short, long, conflicts_with = "multiline")]
        echo: bool,
        /// Read multiline contents until EOF
        #[arg(short, long)]
        multiline: bool,
        /// Overwrite without prompting
        #[arg(short, long)]
        force: bool,
        name: String,
    },
    /// Edit an entry with $EDITOR
    Edit { name: String },
    /// Generate a new password
    Generate {
        /// No symbols, alphanumerics only
        #[arg(short, long)]
        no_symbols: bool,
        /// Copy to clipboard instead of printing
        #[arg(short, long)]
        clip: bool,
        /// Replace only the first line of an existing entry
        #[arg(short, long, conflicts_with = "force")]
        in_place: bool,
        /// Overwrite without prompting
        #[arg(short, long)]
        force: bool,
        name: String,
        length: Option<usize>,
    },
    /// Remove an entry or directory
    #[command(alias = "remove", alias = "delete")]
    Rm {
        #[arg(short, long)]
        recursive: bool,
        #[arg(short, long)]
        force: bool,
        name: String,
    },
    /// Move/rename an entry, re-encrypting for the destination
    #[command(alias = "rename")]
    Mv {
        #[arg(short, long)]
        force: bool,
        old_path: String,
        new_path: String,
    },
    /// Copy an entry, re-encrypting for the destination
    #[command(alias = "copy")]
    Cp {
        #[arg(short, long)]
        force: bool,
        old_path: String,
        new_path: String,
    },
    /// Run a git command inside the store
    #[command(disable_help_flag = true)]
    Git {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Check for a new version and install it
    Update {
        /// Only check; don't install
        #[arg(long)]
        check: bool,
    },
    /// Lock the identity behind authentication (passphrase, security key,
    /// Touch ID)
    Passkey {
        #[command(subcommand)]
        action: PasskeyCmd,
    },
    /// Show every command, what it does, and where your files live
    Help,
}

#[derive(Subcommand)]
enum PasskeyCmd {
    /// Encrypt the identity so every access requires authentication
    Enroll {
        /// Skip the macOS Touch ID slot (passphrase only)
        #[arg(long)]
        no_touchid: bool,
        /// Also enroll a FIDO2 security key (passkey)
        #[arg(long)]
        security_key: bool,
        /// Require the security key's PIN as well as a touch
        #[arg(long, requires = "security_key")]
        pin: bool,
        /// Name for the security key slot
        #[arg(long, value_name = "name", requires = "security_key")]
        label: Option<String>,
    },
    /// Enroll another FIDO2 security key on an already-locked identity
    AddKey {
        /// Name for the new slot (defaults to security-key, security-key-2, …)
        #[arg(long, value_name = "name")]
        label: Option<String>,
        /// Require the security key's PIN as well as a touch
        #[arg(long)]
        pin: bool,
    },
    /// Remove an enrolled security key slot by name
    RemoveKey {
        /// Slot name, as shown by `nopass passkey status`
        label: String,
    },
    /// Remove locking, restoring a plaintext identity (requires auth)
    Disable,
    /// Show whether the identity is locked and which slots exist
    Status,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let store = Store::open(default_store_dir(), build_crypto());

    match cli.command {
        None => show_or_list(&store, cli.name.as_deref().unwrap_or(""), None),
        Some(Cmd::Keygen {
            force,
            identity,
            no_passphrase,
        }) => cmd_keygen(force, identity.as_deref(), no_passphrase),
        Some(Cmd::Ls { subfolder }) => {
            show_or_list(&store, subfolder.as_deref().unwrap_or(""), None)
        }
        Some(Cmd::Show { clip, name }) => show_or_list(&store, name.as_deref().unwrap_or(""), clip),
        Some(Cmd::Init {
            path,
            identity,
            no_passphrase,
            recipients,
        }) => cmd_init(
            &store,
            &path,
            recipients,
            identity.as_deref(),
            no_passphrase,
        ),
        Some(Cmd::Find { terms }) => cmd_find(&store, &terms),
        Some(Cmd::Grep { pattern }) => cmd_grep(&store, &pattern),
        Some(Cmd::Insert {
            echo,
            multiline,
            force,
            name,
        }) => cmd_insert(&store, &name, echo, multiline, force),
        Some(Cmd::Edit { name }) => cmd_edit(&store, &name),
        Some(Cmd::Generate {
            no_symbols,
            clip,
            in_place,
            force,
            name,
            length,
        }) => cmd_generate(&store, &name, length, no_symbols, clip, in_place, force),
        Some(Cmd::Rm {
            recursive,
            force,
            name,
        }) => cmd_rm(&store, &name, recursive, force),
        Some(Cmd::Mv {
            force,
            old_path,
            new_path,
        }) => cmd_copy_move(&store, &old_path, &new_path, true, force),
        Some(Cmd::Cp {
            force,
            old_path,
            new_path,
        }) => cmd_copy_move(&store, &old_path, &new_path, false, force),
        Some(Cmd::Git { args }) => cmd_git(&store, &args),
        Some(Cmd::Update { check }) => cmd_update(check),
        Some(Cmd::Passkey { action }) => cmd_passkey(action),
        Some(Cmd::Help) => cmd_help(),
    }
}

const REPO: &str = "souravsspace/nopass";
const REPO_URL: &str = "https://github.com/souravsspace/nopass";

/// The long-form tour. `--help` lists flags; this explains the tool, and
/// ends with where this machine's files actually are.
fn cmd_help() -> Result<()> {
    println!(
        "\
nopass {version} — a fast, self-contained password manager
{REPO_URL}

Entries are individually encrypted files under one directory, each one
encrypted to your keypair. Reading a password asks for your master
passphrase; writing one never does.

USAGE
  nopass                            list the whole store as a tree
  nopass <entry>                    show an entry
  nopass <command> [options]

SETUP
  init [recipients...]              create your key and store
      -p, --path <subfolder>        (re)initialize just that subfolder
      --identity <path>             where to keep the private key
      --no-passphrase               leave the key unprotected (scripts, CI)
  keygen                            create the keypair without a store
      -f, --force                   replace the existing key

READING
  ls [subfolder]                    list entries as a tree
  show [-c[line]] <entry>           print it, or copy line N to the clipboard
  find <terms>...                   list entries whose names match
  grep <pattern>                    search inside decrypted contents

WRITING
  insert [-e] [-m] [-f] <entry>     add one (echo input / multiline / force)
  generate [-n] [-c] [-i|-f] <entry> [length]
                                    make a password (no symbols / clipboard /
                                    replace first line only / force)
  edit <entry>                      open it in $EDITOR
  rm [-r] [-f] <entry>              delete an entry or directory
  mv [-f] <old> <new>               move, re-encrypting for the destination
  cp [-f] <old> <new>               copy, re-encrypting for the destination

HISTORY AND SYNC
  git init                          start versioning the store
  git <args>...                     any git command, run inside the store
                                    (changes commit and push themselves)

LOCKING THE KEY
  passkey status                    is the key locked, and by what
  passkey enroll                    lock it, or change the passphrase
      --security-key                also enroll a FIDO2 key (e.g. a YubiKey)
      --pin                         require the key's PIN as well as a touch
      --no-touchid                  skip the macOS Touch ID slot
  passkey add-key [--label <name>]  enroll another security key
  passkey remove-key <name>         drop one security key
  passkey disable                   remove the lock, restoring a plain key

MAINTENANCE
  update [--check]                  check for a new release and install it
  help                              this page

ALIASES
  ls=list  rm=remove/delete  mv=rename  cp=copy

ON THIS MACHINE
{locations}
ENVIRONMENT
  NOPASS_DIR        where the store lives
  NOPASS_IDENTITY   where the private key lives (wins over the config file)
  NOPASS_CLIP_TIME  seconds before the clipboard is wiped (default 45)
  NOPASS_UNLOCK     set to \"passphrase\" to skip Touch ID and security keys
                    (full list in the README)

Losing the private key, or forgetting the passphrase, means losing every
password in the store. Back the key up somewhere safe.

Full documentation: {REPO_URL}",
        version = env!("CARGO_PKG_VERSION"),
        locations = locations(),
    );
    Ok(())
}

/// The paths this machine is actually using, so "where is my key?" never
/// needs a trip to the README.
fn locations() -> String {
    let identity = crypto::default_identity_file();
    let state = match std::fs::read_to_string(&identity) {
        Err(_) => "not created yet — run \"nopass init\"",
        Ok(contents) if LockedIdentity::is_locked_file(&contents) => "locked",
        Ok(_) => "unprotected",
    };
    let mut out = format!(
        "  store         {}\n  private key   {} ({state})\n",
        default_store_dir().display(),
        identity.display(),
    );
    let config = nopass_core::config::default_config_file();
    if config.exists() {
        out.push_str(&format!("  config        {}\n", config.display()));
    }
    out
}

/// Parse "1.2.3" into a comparable tuple; non-numeric parts become 0.
fn parse_version(v: &str) -> (u64, u64, u64) {
    let mut parts = v
        .trim()
        .trim_start_matches('v')
        .split('.')
        .map(|p| p.parse().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

/// Latest release tag from GitHub, e.g. "v0.2.0".
fn latest_release_tag() -> Result<String> {
    let out = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "15",
            &format!("https://api.github.com/repos/{REPO}/releases/latest"),
        ])
        .output()
        .context("could not run curl to check for updates")?;
    if !out.status.success() {
        bail!("could not reach GitHub to check for updates (are you online?)");
    }
    let body = String::from_utf8_lossy(&out.stdout);
    body.split("\"tag_name\"")
        .nth(1)
        .and_then(|rest| rest.split('"').nth(1))
        .map(str::to_string)
        .context("no releases published yet")
}

fn installed_via_brew() -> bool {
    Command::new("brew")
        .args(["list", "--formula", "nopass"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn cmd_update(check_only: bool) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    println!("Current version: {current}");
    let latest = latest_release_tag()?;
    println!("Latest release:  {latest}");

    if parse_version(&latest) <= parse_version(current) {
        println!("nopass is up to date.");
        return Ok(());
    }
    if check_only {
        println!("Update available. Run \"nopass update\" to install it.");
        return Ok(());
    }

    if installed_via_brew() {
        println!("Updating via Homebrew...");
        let status = Command::new("brew").args(["upgrade", "nopass"]).status()?;
        if !status.success() {
            bail!("brew upgrade failed");
        }
    } else {
        println!("Updating via cargo (building {latest} from source)...");
        let status = Command::new("cargo")
            .args([
                "install",
                "--git",
                &format!("https://github.com/{REPO}"),
                "--tag",
                &latest,
                "nopass-cli",
                "--force",
            ])
            .status()
            .context("cargo not found; install the new version with your package manager")?;
        if !status.success() {
            bail!("cargo install failed");
        }
    }
    println!("Updated to {latest}.");
    Ok(())
}

fn cmd_keygen(force: bool, identity: Option<&str>, no_passphrase: bool) -> Result<()> {
    let identity_file = setup::target_path(identity)?;
    if identity_file.exists() && !force {
        let existing = crypto::identity_recipient(&identity_file)
            .unwrap_or_else(|| "<locked or unreadable>".to_string());
        bail!(
            "An identity already exists at {} (public key: {existing}).\n\
             Use --force to replace it. Replacing it makes entries encrypted\n\
             only to the old key undecryptable on this machine.",
            identity_file.display()
        );
    }
    setup::create_identity(identity, no_passphrase)?;
    Ok(())
}

/// Recover the current identity secret, unlocking it first if the file is
/// already locked (which prompts for authentication).
fn current_identity_secret(path: &std::path::Path) -> Result<String> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("no identity found at {}", path.display()))?;
    if LockedIdentity::is_locked_file(&contents) {
        let locked = LockedIdentity::parse(&contents)?;
        Ok(auth::CliUnlocker.unlock(&locked)?)
    } else {
        Ok(crypto::read_identity_secret(path)?)
    }
}

fn cmd_passkey(action: PasskeyCmd) -> Result<()> {
    let identity_file = crypto::default_identity_file();
    match action {
        PasskeyCmd::Enroll {
            no_touchid,
            security_key,
            pin,
            label,
        } => cmd_passkey_enroll(&identity_file, no_touchid, security_key, pin, label),
        PasskeyCmd::AddKey { label, pin } => cmd_passkey_add_key(&identity_file, label, pin),
        PasskeyCmd::RemoveKey { label } => cmd_passkey_remove_key(&identity_file, &label),
        PasskeyCmd::Disable => cmd_passkey_disable(&identity_file),
        PasskeyCmd::Status => cmd_passkey_status(&identity_file),
    }
}

fn cmd_passkey_enroll(
    identity_file: &std::path::Path,
    no_touchid: bool,
    security_key: bool,
    want_pin: bool,
    label: Option<String>,
) -> Result<()> {
    if !identity_file.exists() {
        bail!(
            "No identity at {}. Run \"nopass keygen\" or \"nopass init\" first.",
            identity_file.display()
        );
    }
    // Recover the secret (prompts to unlock if already locked).
    let secret = current_identity_secret(identity_file)?;

    // Passphrase slot: always present, the cross-platform lock. Keeping it
    // mandatory is what makes a lost or broken security key survivable.
    let passphrase = prompt_hidden("Choose a passphrase to lock your identity: ")?;
    if passphrase.is_empty() {
        bail!("Error: passphrase must not be empty.");
    }
    let again = prompt_hidden("Retype passphrase: ")?;
    if passphrase != again {
        bail!("Error: the entered passphrases do not match.");
    }
    let passphrase_slot = Some(encrypt_slot(&secret, &SecretString::from(passphrase))?);

    // Optional Touch ID slot (macOS + touchid feature).
    let keychain_slot = enroll_touchid_slot(&secret, no_touchid);

    // Optional FIDO2 security key. Enrolled before anything is written, so a
    // key that refuses leaves the identity exactly as it was.
    let fido2_slots = if security_key {
        vec![enroll_security_key(&secret, &[], label, want_pin)?]
    } else {
        Vec::new()
    };

    let locked = LockedIdentity {
        public: crypto::public_from_secret(&secret),
        passphrase_slot,
        keychain_slot,
        fido2_slots,
    };
    write_identity_file(identity_file, &locked.serialize())?;

    println!("Identity locked. Every access now requires authentication.");
    print_slots(&locked);
    Ok(())
}

/// Register a new security key and seal `secret` to it, returning the slot.
/// `existing` is used only to pick a name that isn't taken.
fn enroll_security_key(
    secret: &str,
    existing: &[Fido2Slot],
    label: Option<String>,
    want_pin: bool,
) -> Result<Fido2Slot> {
    let label = match label {
        Some(label) => {
            if !is_valid_label(&label) {
                bail!(
                    "Error: {label:?} is not a usable slot name (use letters, digits, \
                     '-', '_' or '.', up to 32 characters)."
                );
            }
            if existing.iter().any(|s| s.label == label) {
                bail!("Error: a security key named {label:?} is already enrolled.");
            }
            label
        }
        None => next_key_label(existing),
    };

    let authenticator = fido2::detect().with_context(|| {
        format!(
            "cannot enroll a security key: {}",
            fido2::unavailable_reason()
        )
    })?;

    let pin = if want_pin {
        Some(prompt_hidden("Security key PIN: ")?)
    } else {
        None
    };

    let salt = fido2::random_salt();
    eprintln!(
        "Registering with your {}. You may be asked to touch it twice.",
        authenticator.describe()
    );
    let enrollment = authenticator.enroll(fido2::RP_ID, &salt, pin.as_deref())?;

    Ok(Fido2Slot {
        label,
        rp_id: fido2::RP_ID.to_string(),
        credential_id: enrollment.credential_id,
        salt,
        requires_pin: pin.is_some(),
        ciphertext: encrypt_slot_with_key(secret, &enrollment.secret)?,
    })
}

/// First free name in the security-key, security-key-2, … series.
fn next_key_label(existing: &[Fido2Slot]) -> String {
    let taken = |name: &str| existing.iter().any(|s| s.label == name);
    if !taken("security-key") {
        return "security-key".to_string();
    }
    (2..)
        .map(|n| format!("security-key-{n}"))
        .find(|name| !taken(name))
        .expect("the series is unbounded")
}

fn cmd_passkey_add_key(
    identity_file: &std::path::Path,
    label: Option<String>,
    want_pin: bool,
) -> Result<()> {
    let mut locked = read_locked_identity(identity_file)?;
    // Proving you can already open the identity is what stops a passer-by
    // from adding their own key to your store.
    let secret = auth::CliUnlocker.unlock(&locked)?;

    let slot = enroll_security_key(&secret, &locked.fido2_slots, label, want_pin)?;
    let name = slot.label.clone();
    locked.fido2_slots.push(slot);
    write_identity_file(identity_file, &locked.serialize())?;

    println!("Enrolled security key \"{name}\".");
    print_slots(&locked);
    Ok(())
}

fn cmd_passkey_remove_key(identity_file: &std::path::Path, label: &str) -> Result<()> {
    let mut locked = read_locked_identity(identity_file)?;
    if locked.fido2_slot(label).is_none() {
        bail!("Error: no security key named {label:?} is enrolled.");
    }
    locked.fido2_slots.retain(|s| s.label != label);
    if locked.slot_count() == 0 {
        bail!(
            "Error: {label:?} is the only way to unlock this identity. Run \
             \"nopass passkey disable\" to unlock it instead."
        );
    }
    write_identity_file(identity_file, &locked.serialize())?;

    println!("Removed security key \"{label}\".");
    print_slots(&locked);
    Ok(())
}

/// Read a locked identity file, refusing plaintext ones with a hint.
fn read_locked_identity(identity_file: &std::path::Path) -> Result<LockedIdentity> {
    let contents = std::fs::read_to_string(identity_file)
        .with_context(|| format!("no identity found at {}", identity_file.display()))?;
    if !LockedIdentity::is_locked_file(&contents) {
        bail!("The identity is not locked. Run \"nopass passkey enroll\" first.");
    }
    Ok(LockedIdentity::parse(&contents)?)
}

fn print_slots(locked: &LockedIdentity) {
    println!("Slots:");
    if locked.has_keychain() {
        println!("  - Touch ID (macOS Secure Enclave)");
    }
    for slot in &locked.fido2_slots {
        let pin = if slot.requires_pin { " + PIN" } else { "" };
        println!(
            "  - security key \"{}\" (FIDO2 hmac-secret, credential {}{pin})",
            slot.label,
            short_id(&slot.credential_id),
        );
    }
    if locked.has_passphrase() {
        println!("  - passphrase");
    }
}

/// First few bytes of a credential id, enough to tell two keys apart.
fn short_id(credential_id: &[u8]) -> String {
    credential_id
        .iter()
        .take(4)
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        + "…"
}

#[cfg(all(target_os = "macos", feature = "touchid"))]
fn enroll_touchid_slot(secret: &str, no_touchid: bool) -> Option<Vec<u8>> {
    if no_touchid {
        return None;
    }
    match touchid::enroll_slot(secret) {
        Ok(slot) => Some(slot),
        Err(e) => {
            eprintln!("Skipping Touch ID slot: {e}");
            None
        }
    }
}

#[cfg(not(all(target_os = "macos", feature = "touchid")))]
fn enroll_touchid_slot(_secret: &str, no_touchid: bool) -> Option<Vec<u8>> {
    if !no_touchid {
        eprintln!(
            "Note: Touch ID support is not built in; skipping that slot. \
             (Rebuild with --features touchid on macOS to enable it.)"
        );
    }
    None
}

fn cmd_passkey_disable(identity_file: &std::path::Path) -> Result<()> {
    let locked = read_locked_identity(identity_file)?;
    let secret = auth::CliUnlocker.unlock(&locked)?;
    let public = crypto::write_plaintext_identity(identity_file, &secret)?;
    remove_touchid_slot();
    println!("Identity unlocked and stored in plaintext for {public}.");
    Ok(())
}

#[cfg(all(target_os = "macos", feature = "touchid"))]
fn remove_touchid_slot() {
    if let Err(e) = touchid::remove_key() {
        eprintln!("Note: could not remove the Touch ID key: {e}");
    }
}

#[cfg(not(all(target_os = "macos", feature = "touchid")))]
fn remove_touchid_slot() {}

fn cmd_passkey_status(identity_file: &std::path::Path) -> Result<()> {
    let Ok(contents) = std::fs::read_to_string(identity_file) else {
        println!("No identity found at {}.", identity_file.display());
        return Ok(());
    };
    if !LockedIdentity::is_locked_file(&contents) {
        println!(
            "Identity is unlocked (plaintext on disk). Run \"nopass passkey enroll\" to lock it."
        );
        return Ok(());
    }
    let locked = LockedIdentity::parse(&contents)?;
    println!("Identity is locked.");
    print_slots(&locked);
    Ok(())
}

/// Write `contents` to the identity file with 0600 permissions.
fn write_identity_file(path: &std::path::Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Recipients for init: explicit args win; otherwise use (creating if
/// needed) this machine's own keypair.
fn init_recipients(
    recipients: Vec<String>,
    identity: Option<&str>,
    no_passphrase: bool,
) -> Result<Vec<String>> {
    let recipients: Vec<String> = recipients.into_iter().filter(|r| !r.is_empty()).collect();
    if !recipients.is_empty() {
        return Ok(recipients);
    }
    if std::env::var("NOPASS_BACKEND").as_deref() == Ok("gpg") {
        bail!("Usage: nopass init <gpg-key-id>... (the gpg backend cannot generate keys for you)");
    }
    let identity_file = setup::target_path(identity)?;
    // An existing key is reused as-is — its public half is readable whether
    // or not the secret is locked.
    if let Some(public) = crypto::identity_recipient(&identity_file) {
        return Ok(vec![public]);
    }
    let (_, public) = setup::create_identity(identity, no_passphrase)?;
    println!();
    Ok(vec![public])
}

fn cmd_init(
    store: &Store,
    path: &str,
    recipients: Vec<String>,
    identity: Option<&str>,
    no_passphrase: bool,
) -> Result<()> {
    let recipients = init_recipients(recipients, identity, no_passphrase)?;
    store.init(&recipients, path)?;
    println!("Store initialized for {}", recipients.join(", "));
    Ok(())
}

fn show_or_list(store: &Store, name: &str, clip: Option<usize>) -> Result<()> {
    nopass_core::paths::check_sneaky_path(name).map_err(|e| anyhow::anyhow!("{e}"))?;
    if store.entry_exists(name) {
        let contents = store.show(name)?;
        match clip {
            None => std::io::stdout().write_all(&contents)?,
            Some(line_no) => {
                let text = String::from_utf8_lossy(&contents);
                let line = text
                    .lines()
                    .nth(line_no.saturating_sub(1))
                    .filter(|l| !l.is_empty())
                    .with_context(|| {
                        format!("There is no password to put on the clipboard at line {line_no}.")
                    })?;
                copy_to_clipboard(line, name)?;
            }
        }
    } else if store.dir_exists(name) {
        println!(
            "{}",
            if name.is_empty() {
                "nopass store"
            } else {
                name.trim_end_matches('/')
            }
        );
        print!("{}", store.tree(name)?);
    } else if name.is_empty() {
        bail!("Error: store is empty. Try \"nopass init\".");
    } else {
        bail!("Error: {name} is not in the store.");
    }
    Ok(())
}

fn cmd_find(store: &Store, terms: &[String]) -> Result<()> {
    if terms.is_empty() {
        bail!("Usage: nopass find <terms>...");
    }
    println!("Search Terms: {}", terms.join(","));
    for name in store.find(terms)? {
        println!("{name}");
    }
    Ok(())
}

fn cmd_grep(store: &Store, pattern: &str) -> Result<()> {
    for hit in store.grep(pattern)? {
        println!("{}:", hit.name);
        for line in hit.lines {
            println!("{line}");
        }
    }
    Ok(())
}

fn confirm_overwrite(store: &Store, name: &str, force: bool) -> Result<()> {
    if force || !store.entry_exists(name) {
        return Ok(());
    }
    if !yesno(&format!(
        "An entry already exists for {name}. Overwrite it?"
    ))? {
        std::process::exit(1);
    }
    Ok(())
}

fn cmd_insert(store: &Store, name: &str, echo: bool, multiline: bool, force: bool) -> Result<()> {
    confirm_overwrite(store, name, force)?;

    let contents = if multiline {
        println!("Enter contents of {name} and press Ctrl+D when finished:\n");
        let mut buf = Vec::new();
        std::io::stdin().read_to_end(&mut buf)?;
        buf
    } else if echo || !std::io::stdin().is_terminal() {
        let mut line = String::new();
        if std::io::stdin().is_terminal() {
            eprint!("Enter password for {name}: ");
        }
        std::io::stdin().read_line(&mut line)?;
        format!("{}\n", line.trim_end_matches('\n')).into_bytes()
    } else {
        let pw = prompt_hidden(&format!("Enter password for {name}: "))?;
        let again = prompt_hidden(&format!("Retype password for {name}: "))?;
        if pw != again {
            bail!("Error: the entered passwords do not match.");
        }
        format!("{pw}\n").into_bytes()
    };
    store.insert(name, &contents)?;
    Ok(())
}

fn cmd_edit(store: &Store, name: &str) -> Result<()> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let existing = store.entry_exists(name);
    let tmp = tempdir_secure()?;
    let tmp_file = tmp.join(format!("{}.txt", name.replace('/', "-")));
    if existing {
        std::fs::write(&tmp_file, store.show(name)?)?;
    }

    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg(editor.split_whitespace().next().unwrap_or("vi"))
        .arg(&tmp_file)
        .status()
        .with_context(|| format!("could not launch {editor}"))?;
    if !status.success() {
        bail!("editor exited unsuccessfully");
    }
    if !tmp_file.exists() {
        bail!("New password not saved.");
    }
    let new = std::fs::read(&tmp_file)?;
    std::fs::remove_file(&tmp_file).ok();
    std::fs::remove_dir_all(&tmp).ok();
    if existing && store.show(name)? == new {
        bail!("Password unchanged.");
    }
    store.insert(name, &new)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_generate(
    store: &Store,
    name: &str,
    length: Option<usize>,
    no_symbols: bool,
    clip: bool,
    in_place: bool,
    force: bool,
) -> Result<()> {
    let length = match length {
        Some(n) => n,
        None => std::env::var("NOPASS_GENERATED_LENGTH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(nopass_core::generate::DEFAULT_LENGTH),
    };
    if !in_place {
        confirm_overwrite(store, name, force)?;
    }
    let pass = store.generate(name, length, no_symbols, in_place)?;
    if clip {
        copy_to_clipboard(&pass, name)?;
    } else {
        println!("The generated password for {name} is:\n{pass}");
    }
    Ok(())
}

fn cmd_rm(store: &Store, name: &str, recursive: bool, force: bool) -> Result<()> {
    if !force && !yesno(&format!("Are you sure you would like to delete {name}?"))? {
        std::process::exit(1);
    }
    store.delete(name, recursive)?;
    Ok(())
}

fn cmd_copy_move(store: &Store, old: &str, new: &str, is_move: bool, force: bool) -> Result<()> {
    if !force
        && store.entry_exists(new)
        && std::io::stdin().is_terminal()
        && !yesno(&format!("An entry already exists for {new}. Overwrite it?"))?
    {
        std::process::exit(1);
    }
    store.copy_move(old, new, is_move)?;
    Ok(())
}

fn cmd_git(store: &Store, args: &[String]) -> Result<()> {
    let root = store.root().to_path_buf();
    if args.first().map(String::as_str) == Some("init") {
        let status = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .status()?;
        if !status.success() {
            bail!("git init failed");
        }
        git::add_file(&root, &root, "Add current contents of store.")
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        return Ok(());
    }
    let Some(git_dir) = git::inner_git_dir(&root, &root) else {
        bail!("Error: the store is not a git repository. Try \"nopass git init\".");
    };
    let status = Command::new("git")
        .arg("-C")
        .arg(git_dir)
        .args(args)
        .status()?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

fn yesno(question: &str) -> Result<bool> {
    if !std::io::stdin().is_terminal() {
        return Ok(true);
    }
    eprint!("{question} [y/N] ");
    let mut response = String::new();
    std::io::stdin().read_line(&mut response)?;
    Ok(matches!(response.trim(), "y" | "Y"))
}

/// Read one visible line. An answer is required, so end-of-input is an error
/// rather than a silent default.
fn prompt_line(prompt: &str) -> Result<String> {
    eprint!("{prompt}");
    std::io::stderr().flush().ok();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line)? == 0 {
        bail!("Error: this needs an answer, but there is nothing on stdin to read.");
    }
    Ok(line.trim().to_string())
}

fn prompt_hidden(prompt: &str) -> Result<String> {
    eprint!("{prompt}");
    std::io::stderr().flush().ok();
    // Only worth turning echo off when there is a terminal echoing anything;
    // otherwise stty just prints "stdin isn't a terminal" over the prompt.
    let tty = std::io::stdin().is_terminal();
    let echo = |on: &str| {
        if tty {
            let _ = Command::new("stty").arg(on).status();
        }
    };
    echo("-echo");
    let mut line = String::new();
    let res = std::io::stdin().read_line(&mut line);
    echo("echo");
    eprintln!();
    if res? == 0 {
        bail!("Error: no passphrase given (stdin is empty).");
    }
    Ok(line.trim_end_matches('\n').to_string())
}

fn tempdir_secure() -> Result<PathBuf> {
    // Prefer /dev/shm (ramdisk) when present; fall back to $TMPDIR.
    let base = if std::path::Path::new("/dev/shm").is_dir() {
        PathBuf::from("/dev/shm")
    } else {
        std::env::temp_dir()
    };
    let dir = base.join(format!("nopass.{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn copy_to_clipboard(text: &str, name: &str) -> Result<()> {
    let clip_time: u64 = std::env::var("NOPASS_CLIP_TIME")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(45);

    let copy_cmd = if cfg!(target_os = "macos") {
        vec!["pbcopy"]
    } else if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        vec!["wl-copy"]
    } else {
        vec!["xclip", "-selection", "clipboard"]
    };

    let mut child = Command::new(copy_cmd[0])
        .args(&copy_cmd[1..])
        .stdin(Stdio::piped())
        .spawn()
        .with_context(|| format!("Error: could not run {}", copy_cmd[0]))?;
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(text.as_bytes())?;
    if !child.wait()?.success() {
        bail!("Error: Could not copy data to the clipboard");
    }

    // Detached clearer wipes the clipboard after the timeout.
    let clear = format!("sleep {clip_time}; printf '' | {}", copy_cmd.join(" "));
    Command::new("sh")
        .args(["-c", &clear])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok();
    println!("Copied {name} to clipboard. Will clear in {clip_time} seconds.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_version;

    #[test]
    fn version_parsing_and_ordering() {
        assert_eq!(parse_version("v1.2.3"), (1, 2, 3));
        assert_eq!(parse_version("0.1.0"), (0, 1, 0));
        assert_eq!(parse_version("2"), (2, 0, 0));
        assert!(parse_version("v0.2.0") > parse_version("0.1.9"));
        assert!(parse_version("v0.1.0") <= parse_version("0.1.0"));
        assert!(parse_version("1.0.0") > parse_version("0.99.99"));
    }
}
