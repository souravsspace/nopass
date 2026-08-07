use std::fs;
use std::path::{Path, PathBuf};

use crate::crypto::Crypto;
use crate::error::{Error, Result};
use crate::generate;
use crate::git;
use crate::paths;
use crate::paths::ID_FILE;

/// File extension for encrypted entries.
pub const ENTRY_EXT: &str = "np";

/// A nopass store: a directory tree of encrypted `.np` entries with
/// per-directory `.nopass-id` recipient files and automatic git commits.
pub struct Store {
    root: PathBuf,
    crypto: Box<dyn Crypto>,
}

/// One `grep` result: entry name plus the lines that matched.
pub struct GrepHit {
    pub name: String,
    pub lines: Vec<String>,
}

impl Store {
    pub fn open(root: impl Into<PathBuf>, crypto: Box<dyn Crypto>) -> Self {
        Self {
            root: root.into(),
            crypto,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn entry_file(&self, name: &str) -> PathBuf {
        self.root.join(format!("{name}.{ENTRY_EXT}"))
    }

    pub fn entry_exists(&self, name: &str) -> bool {
        self.entry_file(name).is_file()
    }

    pub fn dir_exists(&self, name: &str) -> bool {
        self.root.join(name).is_dir()
    }

    /// Resolve encryption recipients for a subdirectory: NOPASS_KEY
    /// overrides; otherwise the nearest ancestor recipients file.
    pub fn recipients_for(&self, subdir: &str) -> Result<Vec<String>> {
        if let Ok(keys) = std::env::var("NOPASS_KEY") {
            let keys: Vec<String> = keys.split_whitespace().map(str::to_string).collect();
            if !keys.is_empty() {
                return Ok(keys);
            }
        }
        let file = paths::find_id_file(&self.root, subdir).ok_or(Error::NoRecipients)?;
        let ids = paths::parse_ids(&fs::read_to_string(file)?);
        if ids.is_empty() {
            return Err(Error::NoRecipients);
        }
        Ok(ids)
    }

    /// `init`: write the recipients file at `subpath` and re-encrypt
    /// everything below it.
    pub fn init(&self, recipients: &[String], subpath: &str) -> Result<()> {
        paths::check_sneaky_path(subpath)?;
        let dir = self.root.join(subpath);
        if dir.exists() && !dir.is_dir() {
            return Err(Error::NotADirectory(dir));
        }
        fs::create_dir_all(&dir)?;
        let id_file = dir.join(ID_FILE);
        fs::write(&id_file, format!("{}\n", recipients.join("\n")))?;
        git::add_file(
            &self.root,
            &id_file,
            &format!("Set recipients to {}.", recipients.join(", ")),
        )?;
        self.reencrypt_path(&dir)?;
        git::add_file(
            &self.root,
            &dir,
            &format!("Reencrypt store for recipients {}.", recipients.join(", ")),
        )?;
        Ok(())
    }

    /// Prove the caller owns the store before changing it. Reading does this
    /// on the way past; writing and deleting call it up front.
    pub fn authenticate(&self) -> Result<()> {
        self.crypto.authenticate()
    }

    pub fn show(&self, name: &str) -> Result<Vec<u8>> {
        paths::check_sneaky_path(name)?;
        let file = self.entry_file(name);
        if !file.is_file() {
            return Err(Error::NotInStore(name.to_string()));
        }
        self.crypto.decrypt(&file)
    }

    /// Insert contents for `name`, creating parent dirs and committing.
    pub fn insert(&self, name: &str, contents: &[u8]) -> Result<()> {
        paths::check_sneaky_path(name)?;
        let file = self.entry_file(name);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
        let subdir = parent_subdir(name);
        let recipients = self.recipients_for(&subdir)?;
        self.crypto.encrypt(contents, &recipients, &file)?;
        git::add_file(
            &self.root,
            &file,
            &format!("Add given password for {name} to store."),
        )?;
        Ok(())
    }

    /// Generate and store a password. `in_place` keeps every line but the
    /// first from the existing entry. Returns the generated password.
    pub fn generate(
        &self,
        name: &str,
        length: usize,
        no_symbols: bool,
        in_place: bool,
    ) -> Result<String> {
        paths::check_sneaky_path(name)?;
        let chars = match std::env::var(if no_symbols {
            "NOPASS_CHARACTER_SET_NO_SYMBOLS"
        } else {
            "NOPASS_CHARACTER_SET"
        }) {
            Ok(spec) => generate::charset_from_spec(&spec),
            Err(_) => generate::charset(no_symbols),
        };
        let pass = generate::password(length, &chars)?;

        let file = self.entry_file(name);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
        let recipients = self.recipients_for(&parent_subdir(name))?;

        let contents = if in_place {
            let old = self.crypto.decrypt(&file)?;
            let old_text = String::from_utf8_lossy(&old);
            let rest: Vec<&str> = old_text.lines().skip(1).collect();
            if rest.is_empty() {
                format!("{pass}\n")
            } else {
                format!("{pass}\n{}\n", rest.join("\n"))
            }
        } else {
            format!("{pass}\n")
        };
        self.crypto
            .encrypt(contents.as_bytes(), &recipients, &file)?;
        let verb = if in_place { "Replace" } else { "Add" };
        git::add_file(
            &self.root,
            &file,
            &format!("{verb} generated password for {name}."),
        )?;
        Ok(pass)
    }

    /// Remove an entry (or directory with `recursive`), then clean up
    /// any directories the removal left empty.
    pub fn delete(&self, name: &str, recursive: bool) -> Result<()> {
        paths::check_sneaky_path(name)?;
        let dir = self.root.join(name.trim_end_matches('/'));
        let file = self.entry_file(name);

        let target = if (file.is_file() && dir.is_dir() && name.ends_with('/')) || !file.is_file() {
            dir
        } else {
            file
        };
        if !target.exists() {
            return Err(Error::NotInStore(name.to_string()));
        }
        if target.is_dir() {
            if !recursive {
                return Err(Error::Io(std::io::Error::other(format!(
                    "{name} is a directory; use --recursive"
                ))));
            }
            fs::remove_dir_all(&target)?;
        } else {
            fs::remove_file(&target)?;
        }
        git::rm_and_commit(&self.root, &target, &format!("Remove {name} from store."))?;
        remove_empty_parents(&self.root, target.parent());
        Ok(())
    }

    /// `mv`/`cp`. Trailing slash on `new` (or an existing dir) means "into
    /// that directory". Re-encrypts moved files for destination recipients.
    pub fn copy_move(&self, old: &str, new: &str, is_move: bool) -> Result<()> {
        paths::check_sneaky_paths([old, new])?;
        let old_trimmed = old.trim_end_matches('/');
        let mut old_path = self.root.join(old_trimmed);
        let mut old_is_dir = true;

        let old_entry = self.entry_file(old_trimmed);
        if !(old_entry.is_file() && old_path.is_dir() && old.ends_with('/')) && old_entry.is_file()
        {
            old_path = old_entry;
            old_is_dir = false;
        }
        if !old_path.exists() {
            return Err(Error::NotInStore(old.to_string()));
        }

        let mut new_path = self.root.join(new);
        if let Some(parent) = new_path.parent() {
            fs::create_dir_all(parent)?;
        }
        // A file moved into an existing/trailing-slash dir keeps its
        // basename; otherwise a file destination gets the entry extension.
        if !old_is_dir {
            if new.ends_with('/') || new_path.is_dir() {
                fs::create_dir_all(&new_path)?;
                new_path = new_path.join(old_path.file_name().unwrap());
            } else {
                new_path = self.root.join(format!("{new}.{ENTRY_EXT}"));
            }
        } else if new_path.is_dir() {
            new_path = new_path.join(old_path.file_name().unwrap());
        }

        if is_move {
            fs::rename(&old_path, &new_path)?;
        } else if old_is_dir {
            copy_dir_recursive(&old_path, &new_path)?;
        } else {
            fs::copy(&old_path, &new_path)?;
        }
        self.reencrypt_path(&new_path)?;

        let action = if is_move { "Rename" } else { "Copy" };
        git::add_file(&self.root, &new_path, &format!("{action} {old} to {new}."))?;
        if is_move {
            git::rm_and_commit(&self.root, &old_path, &format!("Remove {old}."))?;
            remove_empty_parents(&self.root, old_path.parent());
        }
        Ok(())
    }

    /// Decrypt and re-encrypt every entry under `path` for the recipients
    /// that currently apply to its directory.
    pub fn reencrypt_path(&self, path: &Path) -> Result<()> {
        for file in entry_files_under(path) {
            let subdir = file
                .parent()
                .and_then(|p| p.strip_prefix(&self.root).ok())
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let recipients = self.recipients_for(&subdir)?;
            let plaintext = self.crypto.decrypt(&file)?;
            self.crypto.encrypt(&plaintext, &recipients, &file)?;
        }
        Ok(())
    }

    /// All entry names (relative, without extension) under `subfolder`, sorted.
    pub fn list(&self, subfolder: &str) -> Result<Vec<String>> {
        paths::check_sneaky_path(subfolder)?;
        let base = self.root.join(subfolder);
        if !base.is_dir() {
            return Err(Error::NotInStore(subfolder.to_string()));
        }
        let mut names: Vec<String> = entry_files_under(&base)
            .into_iter()
            .filter_map(|f| {
                f.strip_prefix(&self.root).ok().map(|rel| {
                    let rel = rel.to_string_lossy();
                    // Only the one extension: an entry called "notes.np"
                    // is stored as notes.np.np and keeps its own name.
                    rel.strip_suffix(&format!(".{ENTRY_EXT}"))
                        .unwrap_or(&rel)
                        .to_string()
                })
            })
            .collect();
        names.sort();
        Ok(names)
    }

    /// Render the store as a tree rooted at `subfolder`.
    pub fn tree(&self, subfolder: &str) -> Result<String> {
        paths::check_sneaky_path(subfolder)?;
        let base = self.root.join(subfolder);
        if !base.is_dir() {
            return Err(Error::NotInStore(subfolder.to_string()));
        }
        let mut out = String::new();
        render_tree(&base, "", &mut out)?;
        Ok(out)
    }

    /// Entries whose name contains any term (case-insensitive).
    pub fn find(&self, terms: &[String]) -> Result<Vec<String>> {
        let lowered: Vec<String> = terms.iter().map(|t| t.to_lowercase()).collect();
        Ok(self
            .list("")?
            .into_iter()
            .filter(|name| {
                let hay = name.to_lowercase();
                lowered.iter().any(|t| hay.contains(t))
            })
            .collect())
    }

    /// Decrypt every entry and report lines containing `pattern`
    /// (case-insensitive substring).
    pub fn grep(&self, pattern: &str) -> Result<Vec<GrepHit>> {
        let needle = pattern.to_lowercase();
        let mut hits = Vec::new();
        for name in self.list("")? {
            let contents = self.show(&name)?;
            let text = String::from_utf8_lossy(&contents);
            let lines: Vec<String> = text
                .lines()
                .filter(|l| l.to_lowercase().contains(&needle))
                .map(str::to_string)
                .collect();
            if !lines.is_empty() {
                hits.push(GrepHit { name, lines });
            }
        }
        Ok(hits)
    }
}

fn parent_subdir(name: &str) -> String {
    Path::new(name)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// All entry files under `path` (or `path` itself if it is one), skipping
/// .git, sorted for determinism.
fn entry_files_under(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return if path.extension().is_some_and(|e| e == ENTRY_EXT) {
            vec![path.to_path_buf()]
        } else {
            vec![]
        };
    }
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.file_name().to_string_lossy() != ".git")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|x| x == ENTRY_EXT))
        .map(|e| e.into_path())
        .collect();
    files.sort();
    files
}

fn render_tree(dir: &Path, prefix: &str, out: &mut String) -> Result<()> {
    let suffix = format!(".{ENTRY_EXT}");
    let mut entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return false;
            }
            e.path().is_dir() || name.ends_with(&suffix)
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let count = entries.len();
    for (i, entry) in entries.into_iter().enumerate() {
        let last = i + 1 == count;
        let connector = if last { "└── " } else { "├── " };
        let name = entry.file_name().to_string_lossy().into_owned();
        let display = name.strip_suffix(&suffix).unwrap_or(&name);
        out.push_str(&format!("{prefix}{connector}{display}\n"));
        if entry.path().is_dir() {
            let child_prefix = format!("{prefix}{}", if last { "    " } else { "│   " });
            render_tree(&entry.path(), &child_prefix, out)?;
        }
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let to = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir_recursive(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

/// Remove now-empty directories up to (not including) the store root.
fn remove_empty_parents(root: &Path, mut dir: Option<&Path>) {
    while let Some(d) = dir {
        if d == root || !d.starts_with(root) {
            break;
        }
        if fs::remove_dir(d).is_err() {
            break;
        }
        dir = d.parent();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::PlainCrypto;

    fn store() -> (tempfile::TempDir, Store) {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::open(tmp.path(), Box::new(PlainCrypto));
        store.init(&["KEY1".to_string()], "").unwrap();
        (tmp, store)
    }

    #[test]
    fn init_writes_recipients_file() {
        let (tmp, _store) = store();
        assert_eq!(
            fs::read_to_string(tmp.path().join(ID_FILE)).unwrap(),
            "KEY1\n"
        );
    }

    #[test]
    fn insert_show_roundtrip() {
        let (_tmp, store) = store();
        store.insert("web/site", b"hunter2\n").unwrap();
        assert_eq!(store.show("web/site").unwrap(), b"hunter2\n");
    }

    #[test]
    fn show_missing_errors() {
        let (_tmp, store) = store();
        assert!(matches!(store.show("nope"), Err(Error::NotInStore(_))));
    }

    #[test]
    fn sneaky_names_rejected_everywhere() {
        let (_tmp, store) = store();
        assert!(store.insert("../evil", b"x").is_err());
        assert!(store.show("../evil").is_err());
        assert!(store.delete("../evil", false).is_err());
        assert!(store.copy_move("../a", "b", true).is_err());
    }

    #[test]
    fn absolute_names_rejected_everywhere() {
        // Joining an absolute path onto the store root drops the root, so
        // these would otherwise name files anywhere on the machine.
        let (tmp, store) = store();
        let outside = tmp.path().parent().unwrap().join("nopass-outside-test");
        let outside = outside.to_string_lossy().into_owned();

        assert!(store.insert(&outside, b"x").is_err());
        assert!(store.show(&outside).is_err());
        assert!(store.delete(&outside, true).is_err());
        assert!(store.copy_move("a", &outside, true).is_err());
        assert!(store.copy_move(&outside, "b", false).is_err());
        assert!(store.list(&outside).is_err());
        assert!(store.tree(&outside).is_err());
        assert!(!Path::new(&outside).exists());
    }

    #[test]
    fn an_entry_named_after_the_extension_keeps_its_name() {
        // "notes.np" is stored as notes.np.np; stripping the extension too
        // eagerly used to leave "notes", which nothing could then read.
        let (_tmp, store) = store();
        store.insert("notes.np", b"hunter2\n").unwrap();
        assert_eq!(store.list("").unwrap(), vec!["notes.np"]);
        assert_eq!(store.show("notes.np").unwrap(), b"hunter2\n");
        assert_eq!(store.grep("hunter2").unwrap().len(), 1);
    }

    #[test]
    fn generate_in_place_keeps_extra_lines() {
        let (_tmp, store) = store();
        store
            .insert("cred", b"oldpass\nuser: me\nurl: x\n")
            .unwrap();
        let pass = store.generate("cred", 23, false, true).unwrap();
        let shown = String::from_utf8(store.show("cred").unwrap()).unwrap();
        assert_eq!(shown, format!("{pass}\nuser: me\nurl: x\n"));
        assert_eq!(pass.chars().count(), 23);
    }

    #[test]
    fn delete_removes_file_and_empty_dirs() {
        let (tmp, store) = store();
        store.insert("a/b/c/cred", b"x\n").unwrap();
        store.delete("a/b/c/cred", false).unwrap();
        assert!(!tmp.path().join("a").exists());
    }

    #[test]
    fn mv_renames_and_cleans_up() {
        let (tmp, store) = store();
        store.insert("cred1", b"secret\n").unwrap();
        store.copy_move("cred1", "cred2", true).unwrap();
        assert!(store.entry_exists("cred2"));
        assert!(!store.entry_exists("cred1"));

        store.copy_move("cred2", "directory/", true).unwrap();
        assert!(tmp.path().join("directory/cred2.np").is_file());

        store
            .copy_move("directory/cred2", "new dir/cred", true)
            .unwrap();
        assert!(tmp.path().join("new dir/cred.np").is_file());
        assert!(!tmp.path().join("directory").exists());

        store.copy_move("new dir", "another", true).unwrap();
        assert!(tmp.path().join("another/cred.np").is_file());

        assert_eq!(store.show("another/cred").unwrap(), b"secret\n");
    }

    #[test]
    fn cp_keeps_original() {
        let (_tmp, store) = store();
        store.insert("orig", b"v\n").unwrap();
        store.copy_move("orig", "copy", false).unwrap();
        assert!(store.entry_exists("orig"));
        assert_eq!(store.show("copy").unwrap(), b"v\n");
    }

    #[test]
    fn subfolder_recipients_apply() {
        let (tmp, store) = store();
        store.init(&["SUBKEY".to_string()], "work").unwrap();
        store.insert("work/cred", b"x\n").unwrap();
        let raw = fs::read_to_string(tmp.path().join("work/cred.np")).unwrap();
        assert!(raw.starts_with("NOPASS-PLAIN:SUBKEY\n"), "{raw}");
    }

    #[test]
    fn mv_reencrypts_for_destination() {
        let (tmp, store) = store();
        store.init(&["SUBKEY".to_string()], "work").unwrap();
        store.insert("cred", b"x\n").unwrap();
        store.copy_move("cred", "work/cred", true).unwrap();
        let raw = fs::read_to_string(tmp.path().join("work/cred.np")).unwrap();
        assert!(raw.starts_with("NOPASS-PLAIN:SUBKEY\n"), "{raw}");
    }

    #[test]
    fn list_find_grep() {
        let (_tmp, store) = store();
        store.insert("web/github", b"a\nlogin: alice\n").unwrap();
        store.insert("web/gitlab", b"b\n").unwrap();
        store.insert("mail/proton", b"c\nlogin: alice\n").unwrap();

        assert_eq!(
            store.list("").unwrap(),
            vec!["mail/proton", "web/github", "web/gitlab"]
        );
        assert_eq!(
            store.find(&["git".to_string()]).unwrap(),
            vec!["web/github", "web/gitlab"]
        );
        let hits = store.grep("alice").unwrap();
        let names: Vec<_> = hits.iter().map(|h| h.name.as_str()).collect();
        assert_eq!(names, vec!["mail/proton", "web/github"]);
    }

    #[test]
    fn tree_renders_without_extension() {
        let (_tmp, store) = store();
        store.insert("web/site", b"x\n").unwrap();
        let tree = store.tree("").unwrap();
        assert!(tree.contains("└── web"), "{tree}");
        assert!(tree.contains("└── site"), "{tree}");
        assert!(!tree.contains(".np"), "{tree}");
    }
}
