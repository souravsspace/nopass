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
            .env_remove("NOPASS_KEY")
            .env_remove("NOPASS_FIDO2_MOCK")
            .env_remove("NOPASS_UNLOCK");
        cmd
    }

    fn identity(&self) -> std::path::PathBuf {
        self.dir.path().join("identity.txt")
    }

    fn identity_text(&self) -> String {
        std::fs::read_to_string(self.identity()).unwrap()
    }

    /// State file for the software test authenticator. Each distinct path is
    /// a distinct "security key"; the default one stands in for the key the
    /// user carries around.
    fn device(&self, name: &str) -> std::path::PathBuf {
        self.dir.path().join(format!("device-{name}"))
    }

    /// `init` with an unprotected key. Used by the tests that are about
    /// something other than the passphrase; first-run setup is covered on
    /// its own further down.
    fn init_unprotected(&self) {
        self.cmd()
            .args(["init", "--no-passphrase"])
            .assert()
            .success();
    }

    /// A command run with security key `name` plugged in.
    fn cmd_with_key(&self, name: &str) -> Command {
        let mut cmd = self.cmd();
        cmd.env("NOPASS_FIDO2_MOCK", self.device(name));
        cmd
    }
}

#[test]
fn native_keygen_creates_identity_and_prints_public_key() {
    let store = NativeStore::new();
    store
        .cmd()
        .args(["keygen", "--no-passphrase"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Private key:").and(predicate::str::contains("age1")));
    assert!(store.dir.path().join("identity.txt").exists());

    // Refuses to clobber without --force
    store
        .cmd()
        .args(["keygen", "--no-passphrase"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
    store
        .cmd()
        .args(["keygen", "--force", "--no-passphrase"])
        .assert()
        .success();
}

#[test]
fn native_init_bootstraps_keypair_automatically() {
    let store = NativeStore::new();
    // No keygen first: init generates the identity itself.
    store
        .cmd()
        .args(["init", "--no-passphrase"])
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
    store.init_unprotected();
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
    store.init_unprotected();
    store
        .cmd()
        .args(["insert", "-e", "cred"])
        .write_stdin("secret\n")
        .assert()
        .success();

    // Swap in a fresh identity: decryption must fail.
    store
        .cmd()
        .args(["keygen", "--force", "--no-passphrase"])
        .assert()
        .success();
    store.cmd().args(["show", "cred"]).assert().failure();
}

#[test]
fn passkey_enroll_locks_identity_and_gates_access() {
    let store = NativeStore::new();
    store.init_unprotected();
    store
        .cmd()
        .args(["insert", "-e", "cred"])
        .write_stdin("top secret\n")
        .assert()
        .success();

    let identity = store.dir.path().join("identity.txt");
    assert!(
        std::fs::read_to_string(&identity)
            .unwrap()
            .contains("AGE-SECRET-KEY"),
        "identity should start out as plaintext"
    );

    // Lock it behind a passphrase (two prompts: choose + retype).
    store
        .cmd()
        .args(["passkey", "enroll"])
        .write_stdin("master-pass\nmaster-pass\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Every access now requires"));

    // On disk the identity is now a locked file with a passphrase slot.
    let locked = std::fs::read_to_string(&identity).unwrap();
    assert!(locked.starts_with("# nopass-locked v1"), "{locked}");
    assert!(!locked.contains("AGE-SECRET-KEY"), "{locked}");

    // Status reports it as locked.
    store
        .cmd()
        .args(["passkey", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("locked").and(predicate::str::contains("passphrase")));

    // Showing the entry requires the passphrase.
    store
        .cmd()
        .args(["show", "cred"])
        .write_stdin("master-pass\n")
        .assert()
        .success()
        .stdout("top secret\n");

    // Wrong passphrase is rejected.
    store
        .cmd()
        .args(["show", "cred"])
        .write_stdin("wrong-pass\n")
        .assert()
        .failure();

    // Disabling restores plaintext access (no further prompts needed).
    store
        .cmd()
        .args(["passkey", "disable"])
        .write_stdin("master-pass\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("unlocked and stored in plaintext"));
    assert!(std::fs::read_to_string(&identity)
        .unwrap()
        .contains("AGE-SECRET-KEY"));
    store
        .cmd()
        .args(["show", "cred"])
        .assert()
        .success()
        .stdout("top secret\n");
}

#[test]
fn passkey_enroll_rejects_mismatched_passphrases() {
    let store = NativeStore::new();
    store.init_unprotected();
    store
        .cmd()
        .args(["passkey", "enroll"])
        .write_stdin("one\ntwo\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("do not match"));
}

#[test]
fn locked_identity_without_input_does_not_hang() {
    // With no passphrase available on a closed stdin, unlocking fails
    // rather than succeeding or blocking forever.
    let store = NativeStore::new();
    store.init_unprotected();
    store
        .cmd()
        .args(["insert", "-e", "cred"])
        .write_stdin("secret\n")
        .assert()
        .success();
    store
        .cmd()
        .args(["passkey", "enroll"])
        .write_stdin("pw\npw\n")
        .assert()
        .success();

    // Empty stdin -> empty passphrase -> wrong -> failure.
    store
        .cmd()
        .args(["show", "cred"])
        .write_stdin("")
        .assert()
        .failure();
}

// ---- FIDO2 security keys (passkeys) ----
//
// These drive the real enroll/unlock code paths through the software test
// authenticator (NOPASS_FIDO2_MOCK), so everything but the USB transport is
// exercised. The recurring trick is running `show` with *empty stdin*: if a
// command succeeds that way, nothing prompted, which is the whole point of a
// security key.

/// A locked store with one entry and a security key enrolled.
fn store_with_security_key() -> NativeStore {
    let store = NativeStore::new();
    store.init_unprotected();
    store
        .cmd()
        .args(["insert", "-e", "cred"])
        .write_stdin("top secret\n")
        .assert()
        .success();
    store
        .cmd_with_key("main")
        .args(["passkey", "enroll", "--security-key"])
        .write_stdin("master-pass\nmaster-pass\n")
        .assert()
        .success();
    store
}

#[test]
fn security_key_enroll_locks_the_identity_and_records_a_slot() {
    let store = store_with_security_key();

    let locked = store.identity_text();
    assert!(locked.starts_with("# nopass-locked v1"), "{locked}");
    assert!(!locked.contains("AGE-SECRET-KEY"), "{locked}");
    assert!(locked.contains("slot-fido2:"), "{locked}");
    // Both factors are recorded: the key, and the passphrase that survives
    // losing it.
    assert!(locked.contains("slot-passphrase:"), "{locked}");

    store
        .cmd()
        .args(["passkey", "status"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("security key \"security-key\"")
                .and(predicate::str::contains("hmac-secret"))
                .and(predicate::str::contains("passphrase")),
        );
}

#[test]
fn security_key_unlocks_without_any_typing() {
    let store = store_with_security_key();

    // Empty stdin: no passphrase could possibly have been supplied.
    store
        .cmd_with_key("main")
        .args(["show", "cred"])
        .write_stdin("")
        .assert()
        .success()
        .stdout("top secret\n");
}

#[test]
fn without_the_key_the_passphrase_still_works() {
    let store = store_with_security_key();

    // No key plugged in: nopass says so and falls back to the passphrase.
    store
        .cmd()
        .args(["show", "cred"])
        .write_stdin("master-pass\n")
        .assert()
        .success()
        .stdout("top secret\n");

    // …and with neither factor, access is refused rather than granted.
    store
        .cmd()
        .args(["show", "cred"])
        .write_stdin("")
        .assert()
        .failure();
}

#[test]
fn the_passphrase_can_be_forced_while_the_key_is_plugged_in() {
    let store = store_with_security_key();
    store
        .cmd_with_key("main")
        .env("NOPASS_UNLOCK", "passphrase")
        .args(["show", "cred"])
        .write_stdin("master-pass\n")
        .assert()
        .success()
        .stdout("top secret\n");
}

#[test]
fn someone_elses_security_key_does_not_unlock_the_store() {
    let store = store_with_security_key();

    // A different authenticator holds none of our credentials. It must not
    // unlock, and must not be quietly accepted either.
    store
        .cmd_with_key("stranger")
        .args(["show", "cred"])
        .write_stdin("")
        .assert()
        .failure();

    // The rightful owner still gets in with the passphrase.
    store
        .cmd_with_key("stranger")
        .args(["show", "cred"])
        .write_stdin("master-pass\n")
        .assert()
        .success()
        .stdout("top secret\n");
}

#[test]
fn enrolling_without_a_key_present_leaves_the_identity_alone() {
    let store = NativeStore::new();
    store.init_unprotected();
    let before = store.identity_text();

    store
        .cmd()
        .args(["passkey", "enroll", "--security-key"])
        .write_stdin("master-pass\nmaster-pass\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("security key"));

    // A half-locked identity would be a disaster; the file must be untouched.
    assert_eq!(store.identity_text(), before);
    assert!(before.contains("AGE-SECRET-KEY"));
}

#[test]
fn a_key_can_be_added_to_an_already_locked_identity() {
    let store = NativeStore::new();
    store.init_unprotected();
    store
        .cmd()
        .args(["insert", "-e", "cred"])
        .write_stdin("top secret\n")
        .assert()
        .success();
    // Locked with a passphrase only, the way it works today.
    store
        .cmd()
        .args(["passkey", "enroll"])
        .write_stdin("master-pass\nmaster-pass\n")
        .assert()
        .success();

    // Adding a key requires proving you can already open the identity.
    store
        .cmd_with_key("main")
        .args(["passkey", "add-key"])
        .write_stdin("master-pass\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Enrolled security key"));

    store
        .cmd_with_key("main")
        .args(["show", "cred"])
        .write_stdin("")
        .assert()
        .success()
        .stdout("top secret\n");
}

#[test]
fn add_key_refuses_an_unlocked_identity() {
    let store = NativeStore::new();
    store.init_unprotected();
    store
        .cmd_with_key("main")
        .args(["passkey", "add-key"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not locked"));
}

#[test]
fn a_backup_key_can_be_enrolled_and_either_one_opens_the_store() {
    let store = store_with_security_key();

    // The backup key is a different device, so unlocking to enroll it falls
    // back to the passphrase.
    store
        .cmd_with_key("backup")
        .args(["passkey", "add-key", "--label", "spare"])
        .write_stdin("master-pass\n")
        .assert()
        .success();

    for key in ["main", "backup"] {
        store
            .cmd_with_key(key)
            .args(["show", "cred"])
            .write_stdin("")
            .assert()
            .success()
            .stdout("top secret\n");
    }
}

#[test]
fn keys_can_be_removed_by_name() {
    let store = store_with_security_key();
    store
        .cmd_with_key("backup")
        .args(["passkey", "add-key", "--label", "spare"])
        .write_stdin("master-pass\n")
        .assert()
        .success();

    store
        .cmd()
        .args(["passkey", "remove-key", "security-key"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed security key"));

    // The removed key no longer opens anything; the survivor still does.
    store
        .cmd_with_key("main")
        .args(["show", "cred"])
        .write_stdin("")
        .assert()
        .failure();
    store
        .cmd_with_key("backup")
        .args(["show", "cred"])
        .write_stdin("")
        .assert()
        .success()
        .stdout("top secret\n");

    store
        .cmd()
        .args(["passkey", "remove-key", "security-key"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no security key named"));
}

#[test]
fn duplicate_and_invalid_key_names_are_rejected() {
    let store = store_with_security_key();

    store
        .cmd_with_key("backup")
        .args(["passkey", "add-key", "--label", "security-key"])
        .write_stdin("master-pass\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already enrolled"));

    store
        .cmd_with_key("backup")
        .args(["passkey", "add-key", "--label", "not a name"])
        .write_stdin("master-pass\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a usable slot name"));
}

#[test]
fn a_pin_protected_key_asks_for_the_pin_and_falls_back_when_it_is_wrong() {
    let store = NativeStore::new();
    store.init_unprotected();
    store
        .cmd()
        .args(["insert", "-e", "cred"])
        .write_stdin("top secret\n")
        .assert()
        .success();
    // Prompts in order: passphrase, retype, then the key's PIN.
    store
        .cmd_with_key("main")
        .args(["passkey", "enroll", "--security-key", "--pin"])
        .write_stdin("master-pass\nmaster-pass\n1234\n")
        .assert()
        .success();

    store
        .cmd()
        .args(["passkey", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("+ PIN"));

    // Right PIN: in, without touching the passphrase.
    store
        .cmd_with_key("main")
        .args(["show", "cred"])
        .write_stdin("1234\n")
        .assert()
        .success()
        .stdout("top secret\n");

    // Wrong PIN alone is not enough...
    store
        .cmd_with_key("main")
        .args(["show", "cred"])
        .write_stdin("0000\n")
        .assert()
        .failure();

    // ...but it falls through to the passphrase rather than giving up.
    store
        .cmd_with_key("main")
        .args(["show", "cred"])
        .write_stdin("0000\nmaster-pass\n")
        .assert()
        .success()
        .stdout("top secret\n");
}

#[test]
fn disable_clears_every_slot_including_the_security_key() {
    let store = store_with_security_key();

    // Unlocking for `disable` can use the key itself: empty stdin.
    store
        .cmd_with_key("main")
        .args(["passkey", "disable"])
        .write_stdin("")
        .assert()
        .success()
        .stdout(predicate::str::contains("unlocked and stored in plaintext"));

    let identity = store.identity_text();
    assert!(identity.contains("AGE-SECRET-KEY"), "{identity}");
    assert!(!identity.contains("slot-fido2:"), "{identity}");
    store
        .cmd()
        .args(["show", "cred"])
        .assert()
        .success()
        .stdout("top secret\n");
}

// ---- auto-sync: pull + push to the configured remote on every change ----

fn git_env(cmd: &mut Command) -> &mut Command {
    cmd.env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "t@t")
}

/// Bare repo acting as the remote, plus a store wired to it.
fn store_with_remote() -> (TestStore, std::path::PathBuf) {
    let store = TestStore::new();
    let bare = store.dir.path().join("remote.git");
    std::process::Command::new("git")
        .args(["init", "-q", "--bare", "-b", "main"])
        .arg(&bare)
        .status()
        .unwrap();
    git_env(&mut store.cmd())
        .args(["git", "init", "-q", "-b", "main"])
        .assert()
        .success();
    store
        .cmd()
        .args(["git", "remote", "add", "origin"])
        .arg(&bare)
        .assert()
        .success();
    (store, bare)
}

fn remote_log(bare: &std::path::Path) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(bare)
        .args(["log", "--format=%s", "main"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn autosync_pushes_every_change_to_remote() {
    let (store, bare) = store_with_remote();

    git_env(&mut store.cmd())
        .args(["insert", "-e", "cred1"])
        .write_stdin("pw\n")
        .assert()
        .success();
    assert!(remote_log(&bare).contains("Add given password for cred1"));

    git_env(&mut store.cmd())
        .args(["mv", "cred1", "cred2"])
        .assert()
        .success();
    assert!(remote_log(&bare).contains("Rename cred1 to cred2"));

    git_env(&mut store.cmd())
        .args(["rm", "-f", "cred2"])
        .assert()
        .success();
    assert!(remote_log(&bare).contains("Remove cred2 from store"));
}

#[test]
fn autosync_can_be_disabled() {
    let (store, bare) = store_with_remote();
    git_env(&mut store.cmd())
        .env("NOPASS_AUTOSYNC", "0")
        .args(["insert", "-e", "cred1"])
        .write_stdin("pw\n")
        .assert()
        .success();
    assert!(!remote_log(&bare).contains("cred1"));
}

#[test]
fn autosync_pulls_remote_changes_before_pushing() {
    let (store, bare) = store_with_remote();

    // First change reaches the remote.
    git_env(&mut store.cmd())
        .args(["insert", "-e", "cred1"])
        .write_stdin("pw\n")
        .assert()
        .success();

    // A second machine clones, adds an entry, pushes.
    let other = store.dir.path().join("other");
    std::process::Command::new("git")
        .args(["clone", "-q"])
        .arg(&bare)
        .arg(&other)
        .status()
        .unwrap();
    let mut second = Command::cargo_bin("nopass").unwrap();
    git_env(&mut second)
        .env("NOPASS_DIR", &other)
        .env("NOPASS_BACKEND", "plain")
        .args(["insert", "-e", "from-other-machine"])
        .write_stdin("pw2\n")
        .assert()
        .success();

    // First machine makes another change: must rebase on top and push,
    // ending up with both entries everywhere.
    git_env(&mut store.cmd())
        .args(["insert", "-e", "cred3"])
        .write_stdin("pw3\n")
        .assert()
        .success();

    let log = remote_log(&bare);
    assert!(log.contains("from-other-machine"), "{log}");
    assert!(log.contains("cred3"), "{log}");
    assert!(store.exists("from-other-machine.np"));
}

// ---- first run: where the private key goes, and locking it by default ----
//
// These run against an empty HOME with no NOPASS_IDENTITY, which is what a
// new install actually looks like: setup has to ask where the key belongs
// and what protects it, and remember the answer for later commands.

struct FreshMachine {
    dir: tempfile::TempDir,
}

impl FreshMachine {
    fn new() -> Self {
        let machine = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir_all(machine.home()).unwrap();
        machine
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("nopass").unwrap();
        cmd.env("HOME", self.home())
            .env("NOPASS_DIR", self.dir.path().join("store"))
            .env_remove("NOPASS_IDENTITY")
            .env_remove("NOPASS_CONFIG")
            .env_remove("NOPASS_BACKEND")
            .env_remove("NOPASS_KEY")
            .env_remove("NOPASS_FIDO2_MOCK")
            .env_remove("NOPASS_UNLOCK");
        cmd
    }

    fn home(&self) -> std::path::PathBuf {
        self.dir.path().join("home")
    }

    fn default_identity(&self) -> std::path::PathBuf {
        self.home().join(".config/nopass/identity.txt")
    }

    fn config(&self) -> std::path::PathBuf {
        self.home().join(".config/nopass/config")
    }

    /// Complete setup taking the default location, with `passphrase`.
    fn set_up_with(&self, passphrase: &str) {
        self.cmd()
            .arg("init")
            .write_stdin(format!("1\n{passphrase}\n{passphrase}\n"))
            .assert()
            .success();
    }

    fn insert(&self, name: &str, password: &str) {
        self.cmd()
            .args(["insert", "-e", name])
            .write_stdin(format!("{password}\n"))
            .assert()
            .success();
    }
}

/// Whether the identity file is locked. A file claiming to be locked had
/// better not still hold the secret key.
fn is_locked(path: &std::path::Path) -> bool {
    let contents = std::fs::read_to_string(path).unwrap();
    let locked = contents.starts_with("# nopass-locked v1");
    if locked {
        assert!(
            !contents.contains("AGE-SECRET-KEY"),
            "secret key left in the clear at {}:\n{contents}",
            path.display()
        );
    }
    locked
}

#[test]
fn first_run_says_where_the_key_goes_and_locks_it() {
    let machine = FreshMachine::new();
    let default = machine.default_identity();

    machine
        .cmd()
        .arg("init")
        .write_stdin("1\nmaster\nmaster\n")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Where should the private key live?")
                .and(predicate::str::contains(default.display().to_string()))
                .and(predicate::str::contains("Public key:  age1"))
                .and(predicate::str::contains("Back up the private key file")),
        );

    assert!(is_locked(&default), "a fresh key must be locked");
    // The default location needs no config file to be found again.
    assert!(!machine.config().exists());
}

#[test]
fn first_run_can_put_the_key_in_a_directory_of_your_choosing() {
    let machine = FreshMachine::new();
    // Somewhere general-purpose, the way someone answers "Desktop".
    let chosen = machine.dir.path().join("Desktop");
    std::fs::create_dir_all(&chosen).unwrap();

    machine
        .cmd()
        .arg("init")
        .write_stdin(format!("2\n{}\nmaster\nmaster\n", chosen.display()))
        .assert()
        .success()
        .stdout(predicate::str::contains("Remembered:"));

    // nopass makes its own folder rather than dropping a bare identity.txt
    // into the directory it was handed.
    let identity = chosen.join("nopass/identity.txt");
    assert!(
        !chosen.join("identity.txt").exists(),
        "the key must not sit loose in the directory the user named"
    );
    assert!(
        is_locked(&identity),
        "the chosen location should hold the key"
    );
    assert!(!machine.default_identity().exists());

    // The choice is remembered, so later commands find it with no env var.
    let config = std::fs::read_to_string(machine.config()).unwrap();
    assert!(config.contains(&identity.display().to_string()), "{config}");

    machine.insert("gmail", "hunter2");
    machine
        .cmd()
        .args(["show", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success()
        .stdout("hunter2\n");
}

#[test]
fn pressing_enter_takes_the_default_location() {
    let machine = FreshMachine::new();
    machine
        .cmd()
        .arg("init")
        .write_stdin("\nmaster\nmaster\n")
        .assert()
        .success();
    assert!(is_locked(&machine.default_identity()));
}

#[test]
fn a_nonsense_choice_is_rejected_rather_than_guessed() {
    let machine = FreshMachine::new();
    machine
        .cmd()
        .arg("init")
        .write_stdin("7\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not one of the choices"));
    assert!(!machine.default_identity().exists());
}

#[test]
fn setup_with_nothing_on_stdin_explains_itself() {
    let machine = FreshMachine::new();
    machine
        .cmd()
        .arg("init")
        .write_stdin("")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("needs a terminal")
                .and(predicate::str::contains("--no-passphrase")),
        );
    assert!(
        !machine.default_identity().exists(),
        "a failed setup must not leave a key behind"
    );
}

#[test]
fn setup_refuses_an_empty_or_mistyped_passphrase() {
    let machine = FreshMachine::new();
    machine
        .cmd()
        .arg("init")
        .write_stdin("1\n\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("must not be empty"));

    machine
        .cmd()
        .arg("init")
        .write_stdin("1\none\ntwo\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("do not match"));

    assert!(!machine.default_identity().exists());
}

#[test]
fn the_passphrase_is_asked_for_on_every_command() {
    let machine = FreshMachine::new();
    machine.set_up_with("master");
    machine.insert("gmail", "hunter2");

    // Reading it works with the passphrase...
    machine
        .cmd()
        .args(["show", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success()
        .stdout("hunter2\n");

    // ...and the very next command asks again: nothing is cached on disk,
    // so an empty answer cannot read anything.
    machine
        .cmd()
        .args(["show", "gmail"])
        .write_stdin("")
        .assert()
        .failure();

    // A wrong passphrase is refused rather than returning garbage.
    machine
        .cmd()
        .args(["show", "gmail"])
        .write_stdin("not-it\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("authentication failed"));
}

#[test]
fn one_command_asks_only_once_however_many_entries_it_reads() {
    let machine = FreshMachine::new();
    machine.set_up_with("master");
    machine.insert("gmail", "hunter2");
    machine.insert("bank", "hunter2");
    machine.insert("work/vpn", "hunter2");

    // grep decrypts every entry in the store. One passphrase on stdin is
    // all it gets; if it re-asked per entry, the second read would fail.
    machine
        .cmd()
        .args(["grep", "hunter2"])
        .write_stdin("master\n")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("gmail")
                .and(predicate::str::contains("bank"))
                .and(predicate::str::contains("work/vpn")),
        );

    // So does re-initializing, which re-encrypts the whole store.
    machine
        .cmd()
        .arg("init")
        .write_stdin("master\n")
        .assert()
        .success();
}

#[test]
fn writing_a_password_needs_no_passphrase_at_all() {
    // Encryption only needs the public key, exactly as with pass and gpg.
    // The password itself is the only thing on stdin here.
    let machine = FreshMachine::new();
    machine.set_up_with("master");
    machine
        .cmd()
        .args(["insert", "-e", "gmail"])
        .write_stdin("hunter2\n")
        .assert()
        .success();
    machine
        .cmd()
        .args(["generate", "bank", "20"])
        .write_stdin("")
        .assert()
        .success();

    machine
        .cmd()
        .args(["show", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success()
        .stdout("hunter2\n");
}

#[test]
fn moving_an_entry_reencrypts_after_a_single_prompt() {
    let machine = FreshMachine::new();
    machine.set_up_with("master");
    machine.insert("gmail", "hunter2");

    machine
        .cmd()
        .args(["mv", "-f", "gmail", "mail/google"])
        .write_stdin("master\n")
        .assert()
        .success();
    machine
        .cmd()
        .args(["show", "mail/google"])
        .write_stdin("master\n")
        .assert()
        .success()
        .stdout("hunter2\n");
}

#[test]
fn the_identity_flag_places_the_key_and_is_remembered() {
    let machine = FreshMachine::new();
    let path = machine.dir.path().join("vault/work-key.txt");

    machine
        .cmd()
        .args(["init", "--identity", path.to_str().unwrap()])
        .write_stdin("master\nmaster\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Where should").not());

    assert!(is_locked(&path));
    let config = std::fs::read_to_string(machine.config()).unwrap();
    assert!(config.contains(&path.display().to_string()), "{config}");

    machine.insert("gmail", "hunter2");
    machine
        .cmd()
        .args(["show", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success()
        .stdout("hunter2\n");
}

#[test]
fn an_existing_key_is_never_asked_about_again() {
    let machine = FreshMachine::new();
    machine.set_up_with("master");
    let before = std::fs::read_to_string(machine.default_identity()).unwrap();

    // A second init reuses the key: no questions, no new keypair.
    machine
        .cmd()
        .arg("init")
        .write_stdin("master\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Where should").not());
    assert_eq!(
        std::fs::read_to_string(machine.default_identity()).unwrap(),
        before
    );

    // keygen still refuses to replace it by accident.
    machine
        .cmd()
        .arg("keygen")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn unattended_setup_makes_an_unprotected_key_and_says_so() {
    let machine = FreshMachine::new();
    machine
        .cmd()
        .args(["init", "--no-passphrase"])
        .write_stdin("")
        .assert()
        .success()
        .stderr(predicate::str::contains("unprotected private key"));

    assert!(!is_locked(&machine.default_identity()));
    machine.insert("gmail", "hunter2");

    // Nothing to type, ever.
    machine
        .cmd()
        .args(["show", "gmail"])
        .write_stdin("")
        .assert()
        .success()
        .stdout("hunter2\n");
}

#[test]
fn an_unprotected_key_can_be_locked_afterwards_and_unlocked_again() {
    let machine = FreshMachine::new();
    machine
        .cmd()
        .args(["init", "--no-passphrase"])
        .assert()
        .success();
    machine.insert("gmail", "hunter2");

    machine
        .cmd()
        .args(["passkey", "enroll"])
        .write_stdin("master\nmaster\n")
        .assert()
        .success();
    assert!(is_locked(&machine.default_identity()));

    machine
        .cmd()
        .args(["show", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success()
        .stdout("hunter2\n");

    // A locked identity still knows its own public key, so writing a new
    // entry needs no unlocking.
    machine
        .cmd()
        .args(["insert", "-e", "bank"])
        .write_stdin("s3cret\n")
        .assert()
        .success();
    machine
        .cmd()
        .args(["show", "bank"])
        .write_stdin("master\n")
        .assert()
        .success()
        .stdout("s3cret\n");

    machine
        .cmd()
        .args(["passkey", "disable"])
        .write_stdin("master\n")
        .assert()
        .success();
    assert!(!is_locked(&machine.default_identity()));
}

#[test]
fn a_passphrase_can_be_changed_by_enrolling_again() {
    let machine = FreshMachine::new();
    machine.set_up_with("old-pass");
    machine.insert("gmail", "hunter2");

    machine
        .cmd()
        .args(["passkey", "enroll"])
        .write_stdin("old-pass\nnew-pass\nnew-pass\n")
        .assert()
        .success();

    machine
        .cmd()
        .args(["show", "gmail"])
        .write_stdin("new-pass\n")
        .assert()
        .success()
        .stdout("hunter2\n");
    machine
        .cmd()
        .args(["show", "gmail"])
        .write_stdin("old-pass\n")
        .assert()
        .failure();
}

#[cfg(unix)]
#[test]
fn editing_an_entry_asks_once_even_though_it_decrypts_twice() {
    use std::os::unix::fs::PermissionsExt;

    let machine = FreshMachine::new();
    machine.set_up_with("master");
    machine.insert("gmail", "hunter2");

    // An "editor" that replaces the file it is handed.
    let editor = machine.dir.path().join("fake-editor");
    std::fs::write(&editor, "#!/bin/sh\nprintf 'edited\\n' > \"$1\"\n").unwrap();
    std::fs::set_permissions(&editor, std::fs::Permissions::from_mode(0o755)).unwrap();

    machine
        .cmd()
        .env("EDITOR", &editor)
        .args(["edit", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success();

    machine
        .cmd()
        .args(["show", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success()
        .stdout("edited\n");
}

#[test]
fn a_directory_named_nopass_is_used_as_is() {
    // Answering with a folder you already made for nopass should not
    // produce keys/nopass/nopass/identity.txt.
    let machine = FreshMachine::new();
    let chosen = machine.dir.path().join("keys/nopass");

    machine
        .cmd()
        .arg("init")
        .write_stdin(format!("2\n{}\nmaster\nmaster\n", chosen.display()))
        .assert()
        .success();

    assert!(is_locked(&chosen.join("identity.txt")));
    assert!(!chosen.join("nopass").exists(), "no doubled nopass folder");
}

#[test]
fn a_trailing_slash_on_the_identity_flag_means_directory() {
    let machine = FreshMachine::new();
    let chosen = machine.dir.path().join("Vaults");

    machine
        .cmd()
        .args(["init", "--identity", &format!("{}/", chosen.display())])
        .write_stdin("master\nmaster\n")
        .assert()
        .success();

    // Same rule as the wizard: the flag gets nopass its own folder.
    assert!(is_locked(&chosen.join("nopass/identity.txt")));
    assert!(!chosen.join("identity.txt").exists());

    machine.insert("gmail", "hunter2");
    machine
        .cmd()
        .args(["show", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success()
        .stdout("hunter2\n");
}

// ---- the help page ----

#[test]
fn help_lists_every_command_and_links_the_repo() {
    let machine = FreshMachine::new();
    let out = machine.cmd().arg("help").assert().success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();

    for command in [
        "init",
        "keygen",
        "ls",
        "show",
        "find",
        "grep",
        "insert",
        "generate",
        "edit",
        "rm",
        "mv",
        "cp",
        "git",
        "update",
        "passkey status",
        "passkey enroll",
        "passkey add-key",
        "passkey remove-key",
        "passkey disable",
        "help",
    ] {
        assert!(
            text.contains(command),
            "help never mentions {command}:\n{text}"
        );
    }
    assert!(
        text.contains("https://github.com/souravsspace/nopass"),
        "{text}"
    );
    assert!(text.contains(env!("CARGO_PKG_VERSION")), "{text}");
}

#[test]
fn help_says_where_this_machines_files_are() {
    let machine = FreshMachine::new();

    // Before setup it admits there is no key yet.
    machine
        .cmd()
        .arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("not created yet"));

    machine.set_up_with("master");

    // Afterwards it points at the real paths and reports the lock state,
    // without asking for the passphrase to do it.
    machine
        .cmd()
        .arg("help")
        .write_stdin("")
        .assert()
        .success()
        .stdout(
            predicate::str::contains(machine.default_identity().display().to_string())
                .and(predicate::str::contains("(locked)")),
        );
}

#[test]
fn help_reports_an_unprotected_key_as_such() {
    let machine = FreshMachine::new();
    machine
        .cmd()
        .args(["init", "--no-passphrase"])
        .assert()
        .success();
    machine
        .cmd()
        .arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("(unprotected)"));
}

#[test]
fn help_mentions_the_config_file_only_once_there_is_one() {
    let machine = FreshMachine::new();
    let chosen = machine.dir.path().join("Elsewhere");

    machine
        .cmd()
        .arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("config        ").not());

    machine
        .cmd()
        .args(["init", "--identity", chosen.to_str().unwrap()])
        .write_stdin("master\nmaster\n")
        .assert()
        .success();

    machine.cmd().arg("help").assert().success().stdout(
        predicate::str::contains(machine.config().display().to_string()).and(
            predicate::str::contains(chosen.join("nopass/identity.txt").display().to_string()),
        ),
    );
}

#[test]
fn the_short_help_points_at_the_long_one() {
    Command::cargo_bin("nopass")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("nopass help").and(predicate::str::contains(
                "https://github.com/souravsspace/nopass",
            )),
        );
}

// ---- Touch ID slot ----

#[cfg(target_os = "macos")]
#[test]
fn touch_id_is_attempted_on_macos_and_explains_itself_when_it_cannot_work() {
    // Test binaries are never code-signed, so this exercises exactly what a
    // `cargo install` user sees: the slot is skipped, the reason names code
    // signing rather than an OSStatus, and locking still succeeds.
    let machine = FreshMachine::new();
    machine
        .cmd()
        .args(["init", "--no-passphrase"])
        .assert()
        .success();

    let out = machine
        .cmd()
        .args(["passkey", "enroll"])
        .write_stdin("master\nmaster\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("passphrase"));

    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    if stderr.contains("Skipping Touch ID") {
        assert!(
            stderr.contains("not code-signed"),
            "the skip reason should be actionable, got:\n{stderr}"
        );
        assert!(
            !stderr.contains("-34018"),
            "raw OSStatus leaked to the user:\n{stderr}"
        );
    }

    // The identity is locked and usable either way.
    machine.insert("gmail", "hunter2");
    machine
        .cmd()
        .args(["show", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success()
        .stdout("hunter2\n");
}

#[test]
fn no_touchid_skips_the_slot_without_comment() {
    let machine = FreshMachine::new();
    machine
        .cmd()
        .args(["init", "--no-passphrase"])
        .assert()
        .success();
    machine
        .cmd()
        .args(["passkey", "enroll", "--no-touchid"])
        .write_stdin("master\nmaster\n")
        .assert()
        .success()
        .stderr(predicate::str::contains("Touch ID").not());
}

#[test]
fn the_macos_signing_script_refuses_to_run_unconfigured() {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packaging/macos/sign.sh")
        .canonicalize()
        .expect("packaging/macos/sign.sh should exist");

    let out = std::process::Command::new("bash")
        .arg(&script)
        .env_remove("TEAM_ID")
        .env_remove("BUNDLE_ID")
        .env_remove("SIGN_IDENTITY")
        .env_remove("PROFILE")
        .output()
        .unwrap();

    assert!(
        !out.status.success(),
        "it must not sign with no configuration"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("TEAM_ID"), "{stderr}");
}
