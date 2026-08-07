//! Which entries a page is allowed to see.
//!
//! This runs in the host rather than the extension, so a compromised content
//! script cannot widen it. Getting it wrong in the permissive direction hands
//! credentials to the wrong site, so the rejections matter more than the
//! acceptances.

use nopass_host::origin::{host_of, matches, Candidate};

fn entry(name: &str) -> Candidate {
    Candidate { name: name.to_string(), url: None }
}

fn entry_with_url(name: &str, url: &str) -> Candidate {
    Candidate { name: name.to_string(), url: Some(url.to_string()) }
}

#[test]
fn an_https_origin_yields_its_host() {
    assert_eq!(host_of("https://mail.google.com").as_deref(), Some("mail.google.com"));
    assert_eq!(host_of("http://localhost:3000").as_deref(), Some("localhost"));
    assert_eq!(host_of("https://EXAMPLE.com").as_deref(), Some("example.com"));
}

#[test]
fn a_non_web_origin_yields_nothing() {
    for origin in [
        "file:///etc/passwd",
        "chrome://settings",
        "moz-extension://abc/popup.html",
        "javascript:alert(1)",
        "null",
        "",
    ] {
        assert_eq!(host_of(origin), None, "{origin} must not produce a host");
    }
}

#[test]
fn an_exact_host_matches() {
    assert!(matches(&entry("web/google.com"), "google.com"));
}

#[test]
fn a_subdomain_of_the_page_matches_a_parent_entry() {
    assert!(matches(&entry("web/google.com"), "mail.google.com"));
    assert!(matches(&entry("web/google.com"), "a.b.google.com"));
}

#[test]
fn a_lookalike_domain_does_not_match() {
    // The classic: an attacker registers a domain that merely *contains* the
    // real one. `google.com.evil.tld` ends with `evil.tld`, not `google.com`.
    assert!(!matches(&entry("web/google.com"), "google.com.evil.tld"));
    assert!(!matches(&entry("web/google.com"), "notgoogle.com"));
    assert!(!matches(&entry("web/google.com"), "google.com.br"));
    assert!(!matches(&entry("web/google.com"), "evil-google.com"));
}

#[test]
fn a_parent_page_does_not_match_a_subdomain_entry() {
    // An entry for `mail.google.com` is not offered on `google.com`: the
    // narrower entry was stored deliberately.
    assert!(!matches(&entry("web/mail.google.com"), "google.com"));
}

#[test]
fn a_bare_suffix_entry_never_matches_everything() {
    for name in ["web/com", "com", "web/co.uk"] {
        assert!(!matches(&entry(name), "google.com"), "{name} must not match google.com");
    }
}

#[test]
fn the_url_field_wins_over_the_entry_name() {
    let candidate = entry_with_url("logins/work-mail", "https://mail.example.org/login");
    assert!(matches(&candidate, "mail.example.org"));
    assert!(!matches(&candidate, "work-mail"));
}

#[test]
fn only_the_last_path_segment_of_a_name_is_a_host() {
    assert!(matches(&entry("web/personal/github.com"), "github.com"));
    assert!(!matches(&entry("web/personal/github.com"), "personal"));
    assert!(!matches(&entry("web/personal/github.com"), "web"));
}

#[test]
fn matching_ignores_case() {
    assert!(matches(&entry("web/GitHub.com"), "github.com"));
}

#[test]
fn a_trailing_dot_does_not_smuggle_a_mismatch_through() {
    // `google.com.` is the same name as `google.com` to a resolver, so the
    // two must not be allowed to disagree here.
    assert!(matches(&entry("web/google.com"), "google.com."));
}
