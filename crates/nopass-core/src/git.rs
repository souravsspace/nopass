use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error::{Error, Result};

/// Find the innermost directory at or above `path` (but inside `store_root`)
/// that is part of a git work tree.
pub fn inner_git_dir(store_root: &Path, path: &Path) -> Option<PathBuf> {
    let mut dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    };
    while !dir.is_dir() && dir.starts_with(store_root) {
        dir = dir.parent()?.to_path_buf();
    }
    if !dir.starts_with(store_root) {
        return None;
    }
    let ok = Command::new("git")
        .arg("-C")
        .arg(&dir)
        .args(["rev-parse", "--is-inside-work-tree"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false);
    ok.then_some(dir)
}

pub fn run(git_dir: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .arg("-C")
        .arg(git_dir)
        .args(args)
        .stdout(Stdio::null())
        .status()?;
    if !status.success() {
        return Err(Error::Git(format!("git {} failed", args.join(" "))));
    }
    Ok(())
}

fn has_changes(git_dir: &Path, path: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(git_dir)
        .args(["status", "--porcelain"])
        .arg(path)
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false)
}

/// `git add` the path and commit if it changed anything. No-op when the
/// store is not a git repo. Auto-syncs with the remote afterwards.
pub fn add_file(store_root: &Path, path: &Path, message: &str) -> Result<()> {
    let Some(git_dir) = inner_git_dir(store_root, path) else {
        return Ok(());
    };
    run(&git_dir, &["add", &path.to_string_lossy()])?;
    if has_changes(&git_dir, path) {
        commit(&git_dir, message)?;
        sync(&git_dir);
    }
    Ok(())
}

/// `git rm -qr` the path (ignoring errors) and commit.
pub fn rm_and_commit(store_root: &Path, path: &Path, message: &str) -> Result<()> {
    let Some(git_dir) = inner_git_dir(store_root, path) else {
        return Ok(());
    };
    let _ = Command::new("git")
        .arg("-C")
        .arg(&git_dir)
        .args(["rm", "-qr", "--ignore-unmatch"])
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if has_changes(&git_dir, path) {
        commit(&git_dir, message)?;
        sync(&git_dir);
    }
    Ok(())
}

pub fn commit(git_dir: &Path, message: &str) -> Result<()> {
    run(git_dir, &["commit", "-m", message])
}

fn git_output(git_dir: &Path, args: &[&str]) -> Option<String> {
    Command::new("git")
        .arg("-C")
        .arg(git_dir)
        .args(args)
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Auto-sync is on by default whenever a remote is configured. Disable
/// with NOPASS_AUTOSYNC=0 or `git config nopass.autosync false`.
fn autosync_enabled(git_dir: &Path) -> bool {
    match std::env::var("NOPASS_AUTOSYNC").as_deref() {
        Ok("0") | Ok("false") | Ok("off") | Ok("no") => return false,
        Ok("1") | Ok("true") | Ok("on") | Ok("yes") => return true,
        _ => {}
    }
    git_output(git_dir, &["config", "--get", "nopass.autosync"]).as_deref() != Some("false")
}

/// Best-effort pull --rebase + push after a commit, when a remote exists.
/// Never fails the password operation; sync problems print a warning.
pub fn sync(git_dir: &Path) {
    if !autosync_enabled(git_dir) {
        return;
    }
    let Some(remote) = git_output(git_dir, &["remote"])
        .and_then(|r| r.lines().next().map(str::to_string))
        .filter(|r| !r.is_empty())
    else {
        return;
    };
    let Some(branch) = git_output(git_dir, &["rev-parse", "--abbrev-ref", "HEAD"]) else {
        return;
    };

    // Pull first so the push lands on top of other machines' changes.
    // Failure is fine (e.g. remote branch doesn't exist yet on first push).
    let _ = Command::new("git")
        .arg("-C")
        .arg(git_dir)
        .args(["pull", "--rebase", "-q", &remote, &branch])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let pushed = Command::new("git")
        .arg("-C")
        .arg(git_dir)
        .args(["push", "-q", "-u", &remote, &branch])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !pushed {
        eprintln!(
            "Warning: could not push to {remote}/{branch}; changes are committed locally. \
             Run \"nopass git push\" when back online."
        );
    }
}
