//! The `nopass-host` binary.
//!
//! With no arguments it serves the native messaging protocol on stdio. The
//! browser launches it that way and passes the calling extension's origin as
//! an argument, so anything that is not a recognised subcommand means *serve*.

use std::io::{stdin, stdout, BufWriter};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use nopass_core::{crypto::NativeCrypto, default_store_dir, Crypto, Store};
use nopass_host::manifest::{self, Browser};
use nopass_host::session::AgentOnly;
use nopass_host::{frame, Host};

#[derive(Parser)]
#[command(
    name = "nopass-host",
    about = "Native messaging host for the nopass browser extension",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Register this host with a browser so its extension can reach it
    Install {
        /// chrome, chromium, brave, edge, firefox, or all
        #[arg(long, default_value = "all")]
        browser: String,
        /// The Chromium extension ID, or the Gecko add-on ID for Firefox
        #[arg(long)]
        extension_id: String,
        /// Path to record for this binary; defaults to where it is now
        #[arg(long)]
        host_path: Option<PathBuf>,
    },
    /// Remove this host's registration
    Uninstall {
        /// chrome, chromium, brave, edge, firefox, or all
        #[arg(long, default_value = "all")]
        browser: String,
    },
}

fn main() -> Result<()> {
    let subcommand = std::env::args().nth(1);
    match subcommand.as_deref() {
        Some("install" | "uninstall" | "--help" | "-h" | "--version" | "-V") => {
            run_command(Cli::parse().command)
        }
        // Launched by the browser: the argument is the calling origin.
        _ => serve(),
    }
}

fn browsers(selector: &str) -> Result<Vec<Browser>> {
    if selector == "all" {
        return Ok(Browser::ALL.to_vec());
    }
    let browser = Browser::parse(selector)
        .with_context(|| format!("unknown browser `{selector}`"))?;
    Ok(vec![browser])
}

fn home() -> Result<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from).context("HOME is not set")
}

fn run_command(command: Cmd) -> Result<()> {
    match command {
        Cmd::Install { browser, extension_id, host_path } => {
            let binary = match host_path {
                Some(path) => path,
                None => std::env::current_exe().context("could not locate this binary")?,
            };
            let home = home()?;

            for browser in browsers(&browser)? {
                match manifest::install(browser, &home, &binary, &extension_id) {
                    Ok(path) => println!("{}: {}", browser.name(), path.display()),
                    // A browser that is not installed is not a failure; the
                    // default is to register with all of them.
                    Err(error) => eprintln!("{}: skipped ({error})", browser.name()),
                }
            }
        }
        Cmd::Uninstall { browser } => {
            let home = home()?;
            for browser in browsers(&browser)? {
                match manifest::uninstall(browser, &home)? {
                    Some(path) => println!("{}: removed {}", browser.name(), path.display()),
                    None => println!("{}: nothing to remove", browser.name()),
                }
            }
        }
    }
    Ok(())
}

/// Read frames until the browser closes the pipe.
///
/// A request that cannot be answered still gets a reply, because the
/// extension is holding a promise against every id it sent. Only a broken
/// frame ends the loop: at that point the stream is out of step and there is
/// no way back to a boundary.
fn serve() -> Result<()> {
    let crypto: Box<dyn Crypto> = Box::new(NativeCrypto::new().with_unlocker(Box::new(AgentOnly)));
    let mut host = Host::new(Store::open(default_store_dir(), crypto));

    let mut input = stdin().lock();
    let mut output = BufWriter::new(stdout().lock());

    while let Some(request) = frame::read_frame(&mut input)? {
        let response = host.handle(&request);
        frame::write_frame(&mut output, &response)?;
    }
    Ok(())
}
