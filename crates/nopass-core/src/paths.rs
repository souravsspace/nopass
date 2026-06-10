use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Name of the per-directory recipients file.
pub const ID_FILE: &str = ".nopass-id";

/// Reject paths that try to escape the store via `..` components.
pub fn check_sneaky_path(path: &str) -> Result<()> {
    let sneaky =
        path == ".." || path.starts_with("../") || path.ends_with("/..") || path.contains("/../");
    if sneaky {
        return Err(Error::SneakyPath);
    }
    Ok(())
}

pub fn check_sneaky_paths<'a, I: IntoIterator<Item = &'a str>>(paths: I) -> Result<()> {
    for p in paths {
        check_sneaky_path(p)?;
    }
    Ok(())
}

/// Walk up from `store_root/subdir` to `store_root` looking for the nearest
/// recipients file, so subfolders can have their own recipients.
pub fn find_id_file(store_root: &Path, subdir: &str) -> Option<PathBuf> {
    let mut current = store_root.join(subdir);
    loop {
        let candidate = current.join(ID_FILE);
        if candidate.is_file() {
            return Some(candidate);
        }
        if current == store_root {
            return None;
        }
        match current.parent() {
            Some(parent) if current.starts_with(store_root) && parent.starts_with(store_root) => {
                current = parent.to_path_buf();
            }
            _ => return None,
        }
    }
}

/// Parse a recipients file: one recipient per line, `#` comments stripped.
pub fn parse_ids(contents: &str) -> Vec<String> {
    contents
        .lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sneaky_paths_rejected() {
        for bad in ["..", "../x", "x/../y", "x/.."] {
            assert!(check_sneaky_path(bad).is_err(), "{bad} should be sneaky");
        }
    }

    #[test]
    fn normal_paths_allowed() {
        for ok in ["a", "a/b", "a/b.c", "..a", "a..", "a..b/c", ".hidden"] {
            assert!(check_sneaky_path(ok).is_ok(), "{ok} should be fine");
        }
    }

    #[test]
    fn id_parsing_strips_comments_and_blanks() {
        let ids = parse_ids("AAAA # main key\n\n# comment only\nBBBB\n");
        assert_eq!(ids, vec!["AAAA", "BBBB"]);
    }

    #[test]
    fn nearest_id_file_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("sub/deep")).unwrap();
        std::fs::write(root.join(ID_FILE), "ROOT\n").unwrap();
        std::fs::write(root.join("sub").join(ID_FILE), "SUB\n").unwrap();

        assert_eq!(
            find_id_file(root, "sub/deep").unwrap(),
            root.join("sub").join(ID_FILE)
        );
        assert_eq!(find_id_file(root, "").unwrap(), root.join(ID_FILE));
        assert_eq!(
            find_id_file(root, "other/place").unwrap(),
            root.join(ID_FILE)
        );
    }

    #[test]
    fn missing_id_file_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(find_id_file(tmp.path(), "anything").is_none());
    }
}
