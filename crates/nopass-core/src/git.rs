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
/// store is not a git repo.
pub fn add_file(store_root: &Path, path: &Path, message: &str) -> Result<()> {
    let Some(git_dir) = inner_git_dir(store_root, path) else {
        return Ok(());
    };
    run(&git_dir, &["add", &path.to_string_lossy()])?;
    if has_changes(&git_dir, path) {
        commit(&git_dir, message)?;
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
    }
    Ok(())
}

pub fn commit(git_dir: &Path, message: &str) -> Result<()> {
    run(git_dir, &["commit", "-m", message])
}
