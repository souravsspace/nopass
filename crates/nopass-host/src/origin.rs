//! Which entries a page is allowed to be offered.
//!
//! This runs in the host, not the extension, so a compromised content script
//! cannot widen it. The rule is deliberately narrow: an entry is offered on
//! its own host and on hosts below it, and nowhere else.

/// An entry as far as matching is concerned: its name, and the `url:` line
/// from its body if one was available.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub name: String,
    pub url: Option<String>,
}

/// The host of a page origin, or `None` when the origin is not a web page.
///
/// Anything that is not `http`/`https` — `file:`, `chrome:`, an extension
/// page, the literal `null` of an opaque origin — has no business being
/// offered credentials, so it gets no host at all.
pub fn host_of(origin: &str) -> Option<String> {
    let rest = origin
        .strip_prefix("https://")
        .or_else(|| origin.strip_prefix("http://"))?;

    let authority = rest.split('/').next()?;
    // Userinfo can carry an `@`; the host is whatever follows the last one.
    let authority = authority.rsplit('@').next()?;

    let host = match authority.strip_prefix('[') {
        // An IPv6 literal is bracketed, and full of the colons that would
        // otherwise look like a port separator.
        Some(bracketed) => bracketed.split(']').next()?,
        None => authority.split(':').next()?,
    };

    normalise(host)
}

/// Lower-case, drop the root label's trailing dot. `google.com.` and
/// `google.com` are the same name to a resolver and must be the same here.
fn normalise(host: &str) -> Option<String> {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

/// The host an entry claims. Its `url:` line wins, because a user who wrote
/// one meant it; otherwise the last path segment of the name is tried, which
/// is the `web/google.com` convention.
fn claimed_host(candidate: &Candidate) -> Option<String> {
    if let Some(url) = &candidate.url {
        return host_of(url).or_else(|| bare_host(url));
    }
    bare_host(candidate.name.rsplit('/').next()?)
}

/// A host written without a scheme, possibly with a port or a path glued on.
fn bare_host(raw: &str) -> Option<String> {
    normalise(raw.split('/').next()?.split(':').next()?)
}

/// Whether `candidate` may be offered on a page served from `page_host`.
pub fn matches(candidate: &Candidate, page_host: &str) -> bool {
    let (Some(page), Some(entry)) = (normalise(page_host), claimed_host(candidate)) else {
        return false;
    };

    // A single label is a public suffix or close enough to one: an entry
    // named `com` must never be offered to every `.com` in existence.
    if !entry.contains('.') {
        return false;
    }

    // Exactly the entry's host, or somewhere beneath it. The leading dot is
    // what keeps `google.com.evil.tld` and `notgoogle.com` out.
    page == entry || page.ends_with(&format!(".{entry}"))
}
