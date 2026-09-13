//! An unlock the agent cannot hold is not an unlock.
//!
//! The browser keeps no cache of its own, so if the agent does not end up
//! holding the passphrase, the very next `status` reads as locked. These are
//! the two ways that happens, both of which used to be reported to the
//! extension as a successful unlock.
//!
//! One test rather than three: the session reads its settings from the
//! environment, and tests in a binary share one.

use nopass_core::crypto::generate_identity_locked;
use nopass_core::lock::SecretString;
use nopass_host::session::Session;
use tempfile::TempDir;

const PASSPHRASE: &str = "correct-horse";

#[test]
fn refuses_an_unlock_it_cannot_keep() {
    let dir = TempDir::new().expect("a temporary directory");
    let identity = dir.path().join("identity.txt");
    let config = dir.path().join("config");
    generate_identity_locked(&identity, &SecretString::from(PASSPHRASE.to_owned()))
        .expect("a locked identity is written");

    // Nowhere to start an agent: the binary named for the cache is not there,
    // which is what a stale or missing `nopass` on PATH amounts to.
    std::env::set_var("NOPASS_CONFIG", &config);
    std::env::set_var("NOPASS_AGENT_SOCK", dir.path().join("agent.sock"));
    std::env::set_var("NOPASS_BIN", dir.path().join("no-such-nopass"));

    // No `cache-ttl` line at all: the shipped default must apply, so the
    // failure is the agent's to report, not the setting's.
    std::fs::write(&config, "identity = /elsewhere/identity.txt\n").expect("the config writes");
    let error = Session::new(identity.clone())
        .unlock(PASSPHRASE)
        .expect_err("an unlock with nowhere to live fails");
    assert!(
        error.to_string().contains("agent"),
        "the shipped default should apply, naming the agent: {error}"
    );
    assert!(
        !error.to_string().contains("cache-ttl"),
        "the shipped default is not an explicit zero: {error}"
    );

    // Explicitly disabled (`cache-ttl = 0`), under which a browser unlock
    // cannot be held at all.
    std::fs::write(&config, "cache-ttl = 0\n").expect("the config writes");
    let error = Session::new(identity.clone())
        .unlock(PASSPHRASE)
        .expect_err("an unlock with nowhere to live fails");
    assert!(
        error.to_string().contains("cache-ttl"),
        "the message should name the setting to change: {error}"
    );

    // Configured, but no agent can be reached or started.
    std::fs::write(&config, "cache-ttl = 900\n").expect("the config writes");
    let error = Session::new(identity.clone())
        .unlock(PASSPHRASE)
        .expect_err("an unlock the agent never took fails");
    assert!(
        error.to_string().contains("agent"),
        "the message should name the agent: {error}"
    );

    // And a wrong passphrase still fails on its own terms, so the extension
    // can tell the user which of the two it is.
    let error = Session::new(identity)
        .unlock("not the passphrase")
        .expect_err("a wrong passphrase fails");
    assert!(
        !error.to_string().contains("agent"),
        "a bad passphrase is not a cache problem: {error}"
    );
}
