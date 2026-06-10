use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use nopass_core::{crypto, default_crypto, default_store_dir, git, Store};

#[derive(Parser)]
#[command(
    name = "nopass",
    version,
    about = "nopass: a fast, self-contained password manager",
    args_conflicts_with_subcommands = true
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
    },
    /// Initialize the store (generates a keypair first if you have none)
    Init {
        /// Subfolder to (re)initialize
        #[arg(short, long, default_value = "")]
        path: String,
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
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let store = Store::open(default_store_dir(), default_crypto());

    match cli.command {
        None => show_or_list(&store, cli.name.as_deref().unwrap_or(""), None),
        Some(Cmd::Keygen { force }) => cmd_keygen(force),
        Some(Cmd::Ls { subfolder }) => {
            show_or_list(&store, subfolder.as_deref().unwrap_or(""), None)
        }
        Some(Cmd::Show { clip, name }) => show_or_list(&store, name.as_deref().unwrap_or(""), clip),
        Some(Cmd::Init { path, recipients }) => cmd_init(&store, &path, recipients),
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
    }
}

fn cmd_keygen(force: bool) -> Result<()> {
    let identity_file = crypto::default_identity_file();
    if identity_file.exists() && !force {
        let existing = crypto::identity_recipient(&identity_file)
            .unwrap_or_else(|| "<unreadable>".to_string());
        bail!(
            "An identity already exists at {} (public key: {existing}).\n\
             Use --force to replace it. Replacing it makes entries encrypted\n\
             only to the old key undecryptable on this machine.",
            identity_file.display()
        );
    }
    let public = crypto::generate_identity(&identity_file).map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("Generated new identity at {}", identity_file.display());
    println!("Public key: {public}");
    Ok(())
}

/// Recipients for init: explicit args win; otherwise use (creating if
/// needed) this machine's own keypair.
fn init_recipients(recipients: Vec<String>) -> Result<Vec<String>> {
    let recipients: Vec<String> = recipients.into_iter().filter(|r| !r.is_empty()).collect();
    if !recipients.is_empty() {
        return Ok(recipients);
    }
    if std::env::var("NOPASS_BACKEND").as_deref() == Ok("gpg") {
        bail!("Usage: nopass init <gpg-key-id>... (the gpg backend cannot generate keys for you)");
    }
    let identity_file = crypto::default_identity_file();
    if let Some(public) = crypto::identity_recipient(&identity_file) {
        return Ok(vec![public]);
    }
    let public = crypto::generate_identity(&identity_file).map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("Generated new identity at {}", identity_file.display());
    println!("Public key: {public}");
    Ok(vec![public])
}

fn cmd_init(store: &Store, path: &str, recipients: Vec<String>) -> Result<()> {
    let recipients = init_recipients(recipients)?;
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

fn prompt_hidden(prompt: &str) -> Result<String> {
    eprint!("{prompt}");
    let _ = Command::new("stty").arg("-echo").status();
    let mut line = String::new();
    let res = std::io::stdin().read_line(&mut line);
    let _ = Command::new("stty").arg("echo").status();
    eprintln!();
    res?;
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
