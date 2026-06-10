//! End-to-end tests for the nopass CLI. Most run hermetically with the
//! plain backend; the native-backend tests exercise real age encryption
//! with a throwaway identity in a temp dir.

use assert_cmd::Command;
use predicates::prelude::*;

const KEY1: &str = "test-recipient-1";

struct TestStore {
    dir: tempfile::TempDir,
}

impl TestStore {
    fn new() -> Self {
        let store = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        store.cmd().args(["init", KEY1]).assert().success();
        store
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("nopass").unwrap();
        cmd.env("NOPASS_DIR", self.dir.path().join("store"))
            .env("NOPASS_BACKEND", "plain")
            .env_remove("NOPASS_KEY")
            .env_remove("NOPASS_GENERATED_LENGTH")
            .env_remove("NOPASS_CHARACTER_SET");
        cmd
    }

    fn insert(&self, name: &str, password: &str) {
        self.cmd()
            .args(["insert", "-e", name])
            .write_stdin(format!("{password}\n"))
            .assert()
            .success();
    }

    fn show(&self, name: &str) -> String {
        let out = self.cmd().args(["show", name]).assert().success();
        String::from_utf8(out.get_output().stdout.clone()).unwrap()
    }

    fn exists(&self, rel: &str) -> bool {
        self.dir.path().join("store").join(rel).exists()
    }
}

#[test]
fn help_runs() {
    Command::cargo_bin("nopass")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("password manager"));
}

#[test]
fn init_writes_recipients_file() {
    let store = TestStore::new();
    assert!(store.exists(".nopass-id"));
    let contents = std::fs::read_to_string(store.dir.path().join("store/.nopass-id")).unwrap();
    assert_eq!(contents.trim(), KEY1);
}

#[test]
fn insert_then_show_roundtrips() {
    let store = TestStore::new();
    store.insert("cred1", "Hello world");
    assert_eq!(store.show("cred1"), "Hello world\n");
}

#[test]
fn show_missing_entry_fails() {
    let store = TestStore::new();
    store
        .cmd()
        .args(["show", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("is not in the store"));
}

#[test]
fn generate_makes_password_of_requested_length() {
    let store = TestStore::new();
    store
        .cmd()
        .args(["generate", "cred", "19"])
        .assert()
        .success();
    let shown = store.show("cred");
    assert_eq!(shown.trim_end_matches('\n').chars().count(), 19);
}

#[test]
fn generate_in_place_replaces_only_first_line() {
    let store = TestStore::new();
    store
        .cmd()
        .args(["insert", "-m", "cred2"])
        .write_stdin("this is a big\npassword\nwith\nmany\nlines\nin it bla bla\n")
        .assert()
        .success();
    store
        .cmd()
        .args(["generate", "-i", "cred2", "23"])
        .assert()
        .success();
    let shown = store.show("cred2");
    let mut lines = shown.lines();
    let first = lines.next().unwrap();
    assert_eq!(first.chars().count(), 23);
    assert_ne!(first, "this is a big");
    assert_eq!(
        lines.collect::<Vec<_>>(),
        vec!["password", "with", "many", "lines", "in it bla bla"]
    );
}

#[test]
fn generate_no_symbols_is_alphanumeric() {
    let store = TestStore::new();
    store
        .cmd()
        .args(["generate", "-n", "alnum", "64"])
        .assert()
        .success();
    let pw = store.show("alnum");
    assert!(pw.trim().chars().all(|c| c.is_ascii_alphanumeric()), "{pw}");
}

#[test]
fn mv_handles_renames_directories_and_cleanup() {
    let store = TestStore::new();
    let initial = "bla bla bla will we make it!!";
    store.insert("cred1", initial);

    // Basic move
    store
        .cmd()
        .args(["mv", "cred1", "cred2"])
        .assert()
        .success();
    assert!(store.exists("cred2.np") && !store.exists("cred1.np"));

    // Move into a new directory (trailing slash)
    store
        .cmd()
        .args(["mv", "cred2", "directory/"])
        .assert()
        .success();
    assert!(store.exists("directory/cred2.np"));

    // Rename into a new dir with spaces; old dir cleaned up when empty
    store
        .cmd()
        .args(["mv", "directory/cred2", "new directory with spaces/cred"])
        .assert()
        .success();
    assert!(store.exists("new directory with spaces/cred.np"));
    assert!(!store.exists("directory"));

    // Directory rename
    store
        .cmd()
        .args(["mv", "new directory with spaces", "anotherdirectory"])
        .assert()
        .success();
    assert!(store.exists("anotherdirectory/cred.np"));
    assert!(!store.exists("new directory with spaces"));

    // Deep directory creation and removal
    store
        .cmd()
        .args(["mv", "anotherdirectory/cred", "new1/new2/new3/new4/thecred"])
        .assert()
        .success();
    store
        .cmd()
        .args(["mv", "new1/new2/new3/new4/thecred", "cred"])
        .assert()
        .success();
    assert!(!store.exists("new1") && store.exists("cred.np"));

    // Contents survived every hop
    assert_eq!(store.show("cred"), format!("{initial}\n"));
}

#[test]
fn rm_deletes_entry() {
    let store = TestStore::new();
    store.insert("doomed", "x");
    store.cmd().args(["rm", "-f", "doomed"]).assert().success();
    assert!(!store.exists("doomed.np"));
}

#[test]
fn rm_directory_requires_recursive() {
    let store = TestStore::new();
    store.insert("dir/one", "x");
    store.cmd().args(["rm", "-f", "dir"]).assert().failure();
    store.cmd().args(["rm", "-rf", "dir"]).assert().success();
    assert!(!store.exists("dir"));
}

#[test]
fn find_filters_by_term() {
    let store = TestStore::new();
    store.insert("web/github", "a");
    store.insert("web/gitlab", "b");
    store.insert("mail/proton", "c");
    store.cmd().args(["find", "git"]).assert().success().stdout(
        predicate::str::contains("web/github")
            .and(predicate::str::contains("web/gitlab"))
            .and(predicate::str::contains("proton").not()),
    );
}

#[test]
fn grep_searches_decrypted_contents() {
    let store = TestStore::new();
    store.insert("a", "login: alice");
    store.insert("b", "login: bob");
    store
        .cmd()
        .args(["grep", "alice"])
        .assert()
        .success()
        .stdout(predicate::str::contains("a:").and(predicate::str::contains("b:").not()));
}

#[test]
fn cp_copies_and_keeps_original() {
    let store = TestStore::new();
    store.insert("orig", "value");
    store.cmd().args(["cp", "orig", "copy"]).assert().success();
    assert!(store.exists("orig.np") && store.exists("copy.np"));
    assert_eq!(store.show("copy"), "value\n");
}

#[test]
fn ls_renders_tree_without_extension() {
    let store = TestStore::new();
    store.insert("web/site", "x");
    store.cmd().assert().success().stdout(
        predicate::str::contains("nopass store")
            .and(predicate::str::contains("site"))
            .and(predicate::str::contains(".np").not()),
    );
}

#[test]
fn sneaky_paths_rejected() {
    let store = TestStore::new();
    store
        .cmd()
        .args(["show", "../../etc/passwd"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("escapes the store"));
}

#[test]
fn empty_uninitialized_store_errors() {
    let dir = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("nopass").unwrap();
    cmd.env("NOPASS_DIR", dir.path().join("missing"))
        .env("NOPASS_BACKEND", "plain")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Try \"nopass init\""));
}

#[test]
fn subfolder_init_changes_recipients_and_mv_reencrypts() {
    let store = TestStore::new();
    store
        .cmd()
        .args(["init", "-p", "work", "SUBKEY"])
        .assert()
        .success();
    store.insert("cred", "secret");
    store
        .cmd()
        .args(["mv", "cred", "work/cred"])
        .assert()
        .success();
    let raw = std::fs::read_to_string(store.dir.path().join("store/work/cred.np")).unwrap();
    assert!(raw.starts_with("NOPASS-PLAIN:SUBKEY\n"), "{raw}");
    assert_eq!(store.show("work/cred"), "secret\n");
}

#[test]
fn git_init_then_mutations_stay_committed() {
    let store = TestStore::new();
    store
        .cmd()
        .args(["git", "init", "-q", "-b", "main"])
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .assert()
        .success();

    for (cmd, args) in [
        ("insert", vec!["insert", "-e", "cred1"]),
        ("mv", vec!["mv", "cred1", "cred2"]),
        ("rm", vec!["rm", "-f", "cred2"]),
    ] {
        let mut c = store.cmd();
        c.args(&args)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "t@t");
        if cmd == "insert" {
            c.write_stdin("pw\n");
        }
        c.assert().success();
    }

    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(store.dir.path().join("store"))
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(
        out.stdout.is_empty(),
        "git not consistent: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

// ---- native backend: real built-in encryption, no external tools ----

struct NativeStore {
    dir: tempfile::TempDir,
}

impl NativeStore {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("nopass").unwrap();
        cmd.env("NOPASS_DIR", self.dir.path().join("store"))
            .env("NOPASS_IDENTITY", self.dir.path().join("identity.txt"))
            .env_remove("NOPASS_BACKEND")
            .env_remove("NOPASS_KEY");
        cmd
    }
}

#[test]
fn native_keygen_creates_identity_and_prints_public_key() {
    let store = NativeStore::new();
    store
        .cmd()
        .arg("keygen")
        .assert()
        .success()
        .stdout(predicate::str::contains("Public key: age1"));
    assert!(store.dir.path().join("identity.txt").exists());

    // Refuses to clobber without --force
    store
        .cmd()
        .arg("keygen")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
    store.cmd().args(["keygen", "--force"]).assert().success();
}

#[test]
fn native_init_bootstraps_keypair_automatically() {
    let store = NativeStore::new();
    // No keygen first: init generates the identity itself.
    store
        .cmd()
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Store initialized for age1"));
    assert!(store.dir.path().join("identity.txt").exists());

    store
        .cmd()
        .args(["insert", "-e", "cred"])
        .write_stdin("native secret\n")
        .assert()
        .success();

    // Ciphertext on disk, plaintext on show.
    let raw = std::fs::read(store.dir.path().join("store/cred.np")).unwrap();
    assert!(!String::from_utf8_lossy(&raw).contains("native secret"));
    store
        .cmd()
        .args(["show", "cred"])
        .assert()
        .success()
        .stdout("native secret\n");
}

#[test]
fn native_generate_and_show_roundtrip() {
    let store = NativeStore::new();
    store.cmd().arg("init").assert().success();
    store
        .cmd()
        .args(["generate", "cred", "31"])
        .assert()
        .success();
    let out = store.cmd().args(["show", "cred"]).assert().success();
    let pw = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert_eq!(pw.trim_end_matches('\n').chars().count(), 31);
}

#[test]
fn native_wrong_identity_cannot_decrypt() {
    let store = NativeStore::new();
    store.cmd().arg("init").assert().success();
    store
        .cmd()
        .args(["insert", "-e", "cred"])
        .write_stdin("secret\n")
        .assert()
        .success();

    // Swap in a fresh identity: decryption must fail.
    store.cmd().args(["keygen", "--force"]).assert().success();
    store.cmd().args(["show", "cred"]).assert().failure();
}
