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
fn absolute_paths_are_refused() {
    // Joining an absolute path onto the store root drops the root, so these
    // would otherwise reach anywhere on the machine the user can write.
    let store = TestStore::new();
    let outside = store.dir.path().join("outside");
    std::fs::create_dir_all(outside.join("keep")).unwrap();
    let outside = outside.display().to_string();

    for args in [
        vec!["rm", "-rf", &outside],
        vec!["show", &outside],
        vec!["ls", &outside],
        vec!["insert", "-e", &outside],
        vec!["mv", "-f", "anything", &outside],
        vec!["cp", "-f", "anything", &outside],
    ] {
        store
            .cmd()
            .args(&args)
            .write_stdin("x\n")
            .assert()
            .failure();
    }
    assert!(std::path::Path::new(&outside).join("keep").is_dir());
}

#[test]
fn clipping_line_zero_is_refused() {
    let store = TestStore::new();
    store.insert("cred", "hunter2");
    store
        .cmd()
        .args(["show", "-c0", "cred"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("line 0"));
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
            // Unsetting NOPASS_CACHE_TTL is not enough to switch the cache
            // off: without this the config falls back to the developer's own
            // `~/.config/nopass/config`, and a `cache-ttl` set there makes a
            // read succeed on any passphrase at all — which is exactly what
            // the tests below are asserting cannot happen.
            .env("NOPASS_CONFIG", self.dir.path().join("config"))
            // A directory nopass has to make itself, so it can insist on
            // one only the owner can reach.
            .env("NOPASS_AGENT_SOCK", self.dir.path().join("run/agent.sock"))
            .env_remove("NOPASS_BACKEND")
            .env_remove("NOPASS_KEY")
            .env_remove("NOPASS_CACHE_TTL")
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

    // Dropping a slot is a change to the lock, so it has to be authorised
    // like every other one. Nothing to answer with, nothing removed.
    store
        .cmd()
        .args(["passkey", "remove-key", "security-key"])
        .write_stdin("")
        .assert()
        .failure();
    store
        .cmd_with_key("main")
        .args(["show", "cred"])
        .write_stdin("")
        .assert()
        .success()
        .stdout("top secret\n");

    store
        .cmd()
        .args(["passkey", "remove-key", "security-key"])
        .write_stdin("master-pass\n")
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
            // A directory nopass has to make itself, so it can insist on
            // one only the owner can reach.
            .env("NOPASS_AGENT_SOCK", self.dir.path().join("run/agent.sock"))
            .env_remove("NOPASS_IDENTITY")
            .env_remove("NOPASS_CONFIG")
            .env_remove("NOPASS_BACKEND")
            .env_remove("NOPASS_KEY")
            .env_remove("NOPASS_CACHE_TTL")
            .env_remove("NOPASS_FIDO2_MOCK")
            .env_remove("NOPASS_UNLOCK");
        cmd
    }

    /// A command allowed to reuse a passphrase cached by the agent for the
    /// next `ttl` seconds. Caching is off unless asked for.
    fn cmd_cached(&self, ttl: u64) -> Command {
        let mut cmd = self.cmd();
        cmd.env("NOPASS_CACHE_TTL", ttl.to_string());
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

    /// Insert into a store whose key is locked. Writing is a change to the
    /// store, so the passphrase is answered first and the password second.
    fn insert_locked(&self, passphrase: &str, name: &str, password: &str) {
        self.cmd()
            .args(["insert", "-e", name])
            .write_stdin(format!("{passphrase}\n{password}\n"))
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

    machine.insert_locked("master", "gmail", "hunter2");
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
    machine.insert_locked("master", "gmail", "hunter2");

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
    machine.insert_locked("master", "gmail", "hunter2");
    machine.insert_locked("master", "bank", "hunter2");
    machine.insert_locked("master", "work/vpn", "hunter2");

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
fn writing_a_password_asks_for_the_passphrase_too() {
    // Encrypting only needs the public key, but adding to the store is a
    // change to it, so nopass proves you are the owner first.
    let machine = FreshMachine::new();
    machine.set_up_with("master");

    // The password alone is not enough: it is read as the passphrase, and
    // then there is nothing left to store.
    machine
        .cmd()
        .args(["insert", "-e", "gmail"])
        .write_stdin("hunter2\n")
        .assert()
        .failure();
    machine
        .cmd()
        .args(["generate", "bank", "20"])
        .write_stdin("")
        .assert()
        .failure();
    assert!(!machine.dir.path().join("store/gmail.np").exists());
    assert!(!machine.dir.path().join("store/bank.np").exists());

    // Passphrase first, then the password.
    machine
        .cmd()
        .args(["insert", "-e", "gmail"])
        .write_stdin("master\nhunter2\n")
        .assert()
        .success();
    machine
        .cmd()
        .args(["generate", "bank", "20"])
        .write_stdin("master\n")
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
fn a_wrong_passphrase_writes_nothing() {
    let machine = FreshMachine::new();
    machine.set_up_with("master");

    machine
        .cmd()
        .args(["insert", "-e", "gmail"])
        .write_stdin("not-it\nhunter2\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("authentication failed"));
    assert!(!machine.dir.path().join("store/gmail.np").exists());
}

#[test]
fn deleting_an_entry_asks_for_the_passphrase() {
    let machine = FreshMachine::new();
    machine.set_up_with("master");
    machine.insert_locked("master", "gmail", "hunter2");

    // Nothing to answer with, so nothing is deleted.
    machine
        .cmd()
        .args(["rm", "-f", "gmail"])
        .write_stdin("")
        .assert()
        .failure();
    assert!(machine.dir.path().join("store/gmail.np").exists());

    machine
        .cmd()
        .args(["rm", "-f", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success();
    assert!(!machine.dir.path().join("store/gmail.np").exists());
}

#[test]
fn moving_an_entry_reencrypts_after_a_single_prompt() {
    let machine = FreshMachine::new();
    machine.set_up_with("master");
    machine.insert_locked("master", "gmail", "hunter2");

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

    machine.insert_locked("master", "gmail", "hunter2");
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

    // Writing a new entry now goes through the same lock: passphrase first,
    // password second.
    machine
        .cmd()
        .args(["insert", "-e", "bank"])
        .write_stdin("master\ns3cret\n")
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
    machine.insert_locked("old-pass", "gmail", "hunter2");

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
    machine.insert_locked("master", "gmail", "hunter2");

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

    machine.insert_locked("master", "gmail", "hunter2");
    machine
        .cmd()
        .args(["show", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success()
        .stdout("hunter2\n");
}

#[test]
fn changing_the_recipients_needs_the_passphrase() {
    // Pointing the store at someone else's key is the most damaging write
    // there is: everything saved afterwards would be encrypted to them.
    let machine = FreshMachine::new();
    machine.set_up_with("master");
    machine.insert_locked("master", "gmail", "hunter2");
    let id_file = machine.dir.path().join("store/.nopass-id");
    let before = std::fs::read_to_string(&id_file).unwrap();

    machine
        .cmd()
        .args([
            "init",
            "age1wy352ryh4jdf93fey3h7ye6cwl5fwj2d54t9jgagl2uwalh6s38sc7guwx",
        ])
        .write_stdin("")
        .assert()
        .failure();

    assert_eq!(std::fs::read_to_string(&id_file).unwrap(), before);
    machine
        .cmd()
        .args(["show", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success()
        .stdout("hunter2\n");
}

#[test]
fn moving_and_copying_without_the_passphrase_change_nothing() {
    let machine = FreshMachine::new();
    machine.set_up_with("master");
    machine.insert_locked("master", "gmail", "hunter2");
    let store = machine.dir.path().join("store");

    for args in [
        vec!["mv", "-f", "gmail", "moved"],
        vec!["cp", "-f", "gmail", "copied"],
    ] {
        machine.cmd().args(&args).write_stdin("").assert().failure();
    }

    assert!(store.join("gmail.np").exists(), "the entry must stay put");
    assert!(!store.join("moved.np").exists());
    assert!(!store.join("copied.np").exists());
}

#[cfg(unix)]
#[test]
fn editing_keeps_the_plaintext_private_and_cleans_up_after_a_failed_editor() {
    use std::os::unix::fs::PermissionsExt;

    let machine = FreshMachine::new();
    machine.set_up_with("master");
    machine.insert_locked("master", "gmail", "hunter2");

    // An "editor" that reports what it was handed, then fails.
    let report = machine.dir.path().join("report");
    let editor = machine.dir.path().join("nosy-editor");
    // `ls -l` rather than `stat`, whose flags mean different things on BSD
    // and GNU: -f asks one for a mode and the other for filesystem stats.
    std::fs::write(
        &editor,
        format!(
            "#!/bin/sh\n\
             ls -ld \"$1\" | cut -c1-10 > {r}\n\
             ls -ld \"$(dirname \"$1\")\" | cut -c1-10 >> {r}\n\
             printf '%s\\n' \"$1\" >> {r}\n\
             exit 1\n",
            r = report.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&editor, std::fs::Permissions::from_mode(0o755)).unwrap();

    machine
        .cmd()
        .env("EDITOR", &editor)
        .args(["edit", "gmail"])
        .write_stdin("master\n")
        .assert()
        .failure();

    let report = std::fs::read_to_string(&report).unwrap();
    let mut lines = report.lines();
    assert_eq!(
        lines.next(),
        Some("-rw-------"),
        "plaintext must be private\n{report}"
    );
    assert_eq!(
        lines.next(),
        Some("drwx------"),
        "its directory too\n{report}"
    );
    let plaintext = lines.next().expect("the editor reports the path");
    assert!(
        !std::path::Path::new(plaintext).exists(),
        "the decrypted entry was left behind at {plaintext}"
    );
}

// ---- the passphrase cache ----

#[cfg(unix)]
#[test]
fn a_cached_passphrase_opens_the_next_read() {
    let machine = FreshMachine::new();
    machine.set_up_with("master");
    machine.insert_locked("master", "gmail", "hunter2");

    // The first read pays for the passphrase and hands it to the agent.
    machine
        .cmd_cached(60)
        .args(["show", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success()
        .stdout("hunter2\n");

    // The next one asks nobody anything.
    machine
        .cmd_cached(60)
        .args(["show", "gmail"])
        .write_stdin("")
        .assert()
        .success()
        .stdout("hunter2\n");

    // A command that has not opted in ignores the cache.
    machine
        .cmd()
        .args(["show", "gmail"])
        .write_stdin("")
        .assert()
        .failure();
}

#[cfg(unix)]
#[test]
fn the_cache_never_satisfies_a_change_to_the_store() {
    let machine = FreshMachine::new();
    machine.set_up_with("master");
    machine.insert_locked("master", "gmail", "hunter2");

    machine
        .cmd_cached(60)
        .args(["show", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success();

    // Reads are warm...
    machine
        .cmd_cached(60)
        .args(["show", "gmail"])
        .write_stdin("")
        .assert()
        .success();

    // ...but every write still wants the passphrase typed.
    machine
        .cmd_cached(60)
        .args(["insert", "-e", "bank"])
        .write_stdin("s3cret\n")
        .assert()
        .failure();
    machine
        .cmd_cached(60)
        .args(["rm", "-f", "gmail"])
        .write_stdin("")
        .assert()
        .failure();
    assert!(machine.dir.path().join("store/gmail.np").exists());
}

#[cfg(unix)]
#[test]
fn lock_forgets_the_cached_passphrase() {
    let machine = FreshMachine::new();
    machine.set_up_with("master");
    machine.insert_locked("master", "gmail", "hunter2");
    machine
        .cmd_cached(60)
        .args(["show", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success();

    machine.cmd().arg("lock").assert().success();

    machine
        .cmd_cached(60)
        .args(["show", "gmail"])
        .write_stdin("")
        .assert()
        .failure();
}

#[cfg(unix)]
#[test]
fn the_cache_expires() {
    let machine = FreshMachine::new();
    machine.set_up_with("master");
    machine.insert_locked("master", "gmail", "hunter2");
    machine
        .cmd_cached(1)
        .args(["show", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success();

    std::thread::sleep(std::time::Duration::from_secs(2));

    machine
        .cmd_cached(60)
        .args(["show", "gmail"])
        .write_stdin("")
        .assert()
        .failure();
}

#[cfg(unix)]
#[test]
fn changing_the_passphrase_leaves_the_old_cache_behind() {
    let machine = FreshMachine::new();
    machine.set_up_with("master");
    machine.insert_locked("master", "gmail", "hunter2");
    machine
        .cmd_cached(60)
        .args(["show", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success();

    machine
        .cmd_cached(60)
        .args(["passkey", "enroll"])
        .write_stdin("master\nnew-pass\nnew-pass\n")
        .assert()
        .success();

    // The cache belongs to the identity as it was, so the relocked one is
    // not readable without typing the new passphrase.
    machine
        .cmd_cached(60)
        .args(["show", "gmail"])
        .write_stdin("")
        .assert()
        .failure();
    machine
        .cmd_cached(60)
        .args(["show", "gmail"])
        .write_stdin("new-pass\n")
        .assert()
        .success()
        .stdout("hunter2\n");
}

#[cfg(unix)]
#[test]
fn a_client_that_says_nothing_does_not_wedge_the_agent() {
    let machine = FreshMachine::new();
    machine.set_up_with("master");
    machine.insert_locked("master", "gmail", "hunter2");
    machine
        .cmd_cached(60)
        .args(["show", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success();

    // Somebody connects and then says nothing at all — a crashed client, or
    // a stray `nc -U`. Everyone else must carry on.
    let _mute = std::os::unix::net::UnixStream::connect(machine.dir.path().join("run/agent.sock"))
        .expect("the agent is listening");

    let started = std::time::Instant::now();
    machine
        .cmd_cached(60)
        .args(["show", "gmail"])
        .write_stdin("")
        .assert()
        .success()
        .stdout("hunter2\n");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "a silent client held everyone else up for {:?}",
        started.elapsed()
    );
}

#[cfg(unix)]
#[test]
fn agent_status_says_whether_anything_is_cached() {
    let machine = FreshMachine::new();
    machine.set_up_with("master");
    machine.insert_locked("master", "gmail", "hunter2");

    machine
        .cmd()
        .args(["agent", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Nothing is cached"));

    machine
        .cmd_cached(60)
        .args(["show", "gmail"])
        .write_stdin("master\n")
        .assert()
        .success();

    machine
        .cmd()
        .args(["agent", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("expires in"));

    machine.cmd().args(["agent", "stop"]).assert().success();
    machine
        .cmd()
        .args(["agent", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Nothing is cached"));
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
        "lock",
        "agent status",
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

#[test]
fn insert_writes_a_card_with_the_fields_it_was_given() {
    let store = TestStore::new();
    store
        .cmd()
        .args([
            "insert",
            "-e",
            "--type",
            "card",
            "--field",
            "cardholder=Sana Qureshi",
            "--field",
            "exp=04/2029",
            "cards/visa",
        ])
        .write_stdin("4111111111111111\n")
        .assert()
        .success();

    let body = store.show("cards/visa");
    assert!(body.starts_with("4111111111111111\n"), "{body}");
    assert!(body.contains("type: card"), "{body}");
    assert!(body.contains("cardholder: Sana Qureshi"), "{body}");
    assert!(body.contains("exp: 04/2029"), "{body}");
}

#[test]
fn insert_writes_an_identity_that_needs_no_secret() {
    let store = TestStore::new();
    store
        .cmd()
        .args([
            "insert",
            "--type",
            "identity",
            "--field",
            "given-name=Sana",
            "--field",
            "country=Bangladesh",
            "me/home",
        ])
        .assert()
        .success();

    let body = store.show("me/home");
    assert!(body.starts_with("\ntype: identity"), "{body}");
    assert!(body.contains("given-name: Sana"), "{body}");
    assert!(body.contains("country: Bangladesh"), "{body}");
}

#[test]
fn a_field_carrying_a_line_break_is_refused() {
    // The same rule the wire keeps: a value free to carry a newline could
    // forge a `url:` line pointing somewhere the user never typed.
    let store = TestStore::new();
    store
        .cmd()
        .args([
            "insert",
            "-e",
            "--field",
            "url=https://a.b\nusername: someone",
            "web/example.com",
        ])
        .write_stdin("hunter2\n")
        .assert()
        .failure();
}

#[test]
fn set_changes_one_field_and_leaves_the_rest_alone() {
    let store = TestStore::new();
    store
        .cmd()
        .args([
            "insert",
            "-e",
            "--field",
            "username=sana",
            "--field",
            "url=https://a.b",
            "web/example.com",
        ])
        .write_stdin("hunter2\n")
        .assert()
        .success();

    store
        .cmd()
        .args(["set", "web/example.com", "username=someone@else"])
        .assert()
        .success();

    let body = store.show("web/example.com");
    assert!(body.starts_with("hunter2\n"), "{body}");
    assert!(body.contains("username: someone@else"), "{body}");
    assert!(body.contains("url: https://a.b"), "{body}");
}

#[test]
fn set_keeps_a_line_it_does_not_understand() {
    let store = TestStore::new();
    store
        .cmd()
        .args(["insert", "-m", "notes/thing"])
        .write_stdin("hunter2\nfavourite-colour: green\n")
        .assert()
        .success();

    store
        .cmd()
        .args(["set", "notes/thing", "username=sana"])
        .assert()
        .success();

    assert!(
        store
            .show("notes/thing")
            .contains("favourite-colour: green"),
        "an unknown line was dropped"
    );
}

#[test]
fn set_writes_a_field_under_the_spelling_the_entry_already_used() {
    let store = TestStore::new();
    store
        .cmd()
        .args(["insert", "-m", "web/example.com"])
        .write_stdin("hunter2\nemail: old@example.com\n")
        .assert()
        .success();

    store
        .cmd()
        .args(["set", "web/example.com", "username=new@example.com"])
        .assert()
        .success();

    let body = store.show("web/example.com");
    assert!(body.contains("email: new@example.com"), "{body}");
    assert!(
        !body.contains("username:"),
        "a second spelling appeared: {body}"
    );
}

#[test]
fn set_can_change_the_secret_itself() {
    let store = TestStore::new();
    store.insert("web/example.com", "hunter2");

    store
        .cmd()
        .args(["set", "web/example.com", "--secret", "new-password"])
        .assert()
        .success();

    assert!(store.show("web/example.com").starts_with("new-password\n"));
}

#[test]
fn set_clears_a_field_written_empty() {
    let store = TestStore::new();
    store
        .cmd()
        .args([
            "insert",
            "-e",
            "--field",
            "username=sana",
            "web/example.com",
        ])
        .write_stdin("hunter2\n")
        .assert()
        .success();

    store
        .cmd()
        .args(["set", "web/example.com", "username="])
        .assert()
        .success();

    assert!(!store.show("web/example.com").contains("username"));
}

#[test]
fn set_refuses_an_entry_that_is_not_there() {
    let store = TestStore::new();
    store
        .cmd()
        .args(["set", "web/nowhere", "username=sana"])
        .assert()
        .failure();
}

#[test]
fn show_can_print_one_field_on_its_own() {
    let store = TestStore::new();
    store
        .cmd()
        .args([
            "insert",
            "-e",
            "--field",
            "username=sana",
            "web/example.com",
        ])
        .write_stdin("hunter2\n")
        .assert()
        .success();

    store
        .cmd()
        .args(["show", "--field", "username", "web/example.com"])
        .assert()
        .success()
        .stdout("sana\n");
}

#[test]
fn show_field_reads_whichever_spelling_the_entry_used() {
    let store = TestStore::new();
    store
        .cmd()
        .args(["insert", "-m", "web/example.com"])
        .write_stdin("hunter2\nemail: sana@example.com\n")
        .assert()
        .success();

    store
        .cmd()
        .args(["show", "--field", "username", "web/example.com"])
        .assert()
        .success()
        .stdout("sana@example.com\n");
}

#[test]
fn show_field_says_so_when_the_entry_has_no_such_field() {
    let store = TestStore::new();
    store.insert("web/example.com", "hunter2");

    store
        .cmd()
        .args(["show", "--field", "totp", "web/example.com"])
        .assert()
        .failure();
}

#[test]
fn webauthn_register_writes_a_passkey_and_prints_what_a_site_expects() {
    let store = TestStore::new();
    let out = store
        .cmd()
        .args([
            "webauthn",
            "register",
            "--rp",
            "example.com",
            "--user",
            "sana@example.com",
            "--challenge",
            "Y2hhbGxlbmdl",
            "keys/example",
        ])
        .assert()
        .success();

    let printed = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    // What a browser hands to `navigator.credentials.create`'s caller.
    for key in [
        "\"type\": \"public-key\"",
        "\"id\":",
        "\"rawId\":",
        "\"attestationObject\":",
        "\"clientDataJSON\":",
    ] {
        assert!(printed.contains(key), "{key} missing from {printed}");
    }

    let body = store.show("keys/example");
    assert!(body.contains("type: passkey"), "{body}");
    assert!(body.contains("rp: example.com"), "{body}");
    assert!(body.contains("user: sana@example.com"), "{body}");
    assert!(body.contains("counter: 0"), "{body}");
    assert!(body.contains("alg: -7"), "{body}");
}

#[test]
fn webauthn_list_shows_the_passkeys_and_not_their_keys() {
    let store = TestStore::new();
    store
        .cmd()
        .args([
            "webauthn",
            "register",
            "--rp",
            "example.com",
            "keys/example",
        ])
        .assert()
        .success();

    let out = store.cmd().args(["webauthn", "list"]).assert().success();
    let printed = String::from_utf8(out.get_output().stdout.clone()).unwrap();

    assert!(printed.contains("keys/example"), "{printed}");
    assert!(printed.contains("example.com"), "{printed}");

    let secret = store
        .show("keys/example")
        .lines()
        .next()
        .unwrap()
        .to_string();
    assert!(!printed.contains(&secret), "list printed the private key");
}

#[test]
fn webauthn_assert_signs_a_challenge_and_verify_accepts_it() {
    let store = TestStore::new();
    store
        .cmd()
        .args([
            "webauthn",
            "register",
            "--rp",
            "example.com",
            "keys/example",
        ])
        .assert()
        .success();

    let signed = store
        .cmd()
        .args([
            "webauthn",
            "assert",
            "--challenge",
            "Y2hhbGxlbmdl",
            "keys/example",
        ])
        .assert()
        .success();
    let printed = String::from_utf8(signed.get_output().stdout.clone()).unwrap();
    assert!(printed.contains("\"signature\":"), "{printed}");
    assert!(printed.contains("\"authenticatorData\":"), "{printed}");

    // `verify` is the relying party's half, run locally so a signature can be
    // watched to hold rather than taken on trust.
    store
        .cmd()
        .args(["webauthn", "verify", "keys/example"])
        .write_stdin(printed)
        .assert()
        .success()
        .stdout(predicate::str::contains("signature is valid"));
}

#[test]
fn webauthn_assert_moves_the_counter_on_every_use() {
    let store = TestStore::new();
    store
        .cmd()
        .args([
            "webauthn",
            "register",
            "--rp",
            "example.com",
            "keys/example",
        ])
        .assert()
        .success();

    for _ in 0..2 {
        store
            .cmd()
            .args([
                "webauthn",
                "assert",
                "--challenge",
                "Y2hhbGxlbmdl",
                "keys/example",
            ])
            .assert()
            .success();
    }

    assert!(
        store.show("keys/example").contains("counter: 2"),
        "the counter did not move: {}",
        store.show("keys/example")
    );
}

#[test]
fn webauthn_verify_refuses_a_tampered_signature() {
    let store = TestStore::new();
    store
        .cmd()
        .args([
            "webauthn",
            "register",
            "--rp",
            "example.com",
            "keys/example",
        ])
        .assert()
        .success();

    let signed = store
        .cmd()
        .args([
            "webauthn",
            "assert",
            "--challenge",
            "Y2hhbGxlbmdl",
            "keys/example",
        ])
        .assert()
        .success();
    let printed = String::from_utf8(signed.get_output().stdout.clone()).unwrap();
    // Swap a character of the signature for another valid base64url one.
    let tampered = printed.replacen("\"signature\": \"M", "\"signature\": \"N", 1);

    store
        .cmd()
        .args(["webauthn", "verify", "keys/example"])
        .write_stdin(tampered)
        .assert()
        .failure();
}

#[test]
fn webauthn_register_refuses_a_name_that_is_taken() {
    let store = TestStore::new();
    store.insert("keys/example", "not-a-passkey");

    store
        .cmd()
        .args([
            "webauthn",
            "register",
            "--rp",
            "example.com",
            "keys/example",
        ])
        .assert()
        .failure();
}

#[test]
fn webauthn_assert_refuses_an_entry_that_is_not_a_passkey() {
    let store = TestStore::new();
    store.insert("web/example.com", "hunter2");

    store
        .cmd()
        .args([
            "webauthn",
            "assert",
            "--challenge",
            "Y2hhbGxlbmdl",
            "web/example.com",
        ])
        .assert()
        .failure();
}
