# ADR-0002: The browser extension is read-only in v1

**Date**: 2026-08-08
**Status**: superseded in part by [ADR-0006](0006-create-only-writes-from-the-extension.md)
**Deciders**: repository owner

> ADR-0006 revisits the one line below that turned out not to hold: creating an
> entry needs only the recipients' public keys, so it needs no passphrase and
> no prompt. `insert` is now accepted, under a create-only gate. Everything
> else here — every verb that reaches an entry already in the store — still
> stands, for the reasons given.

## Context

nopass asks the user to authenticate for **every** mutation. A warm agent cache
authorises reads; anything that changes the store prompts the person at the
keyboard directly. That is a deliberate property, documented in
`crates/nopass-cli/src/agent.rs`: "a warm cache never authorises a write."

A native messaging host is spawned by the browser as a child process with no
controlling terminal. It cannot run that prompt. So the browser bridge either
has to weaken the invariant, invent a new prompt, or not write at all.

## Decision

The v1 host accepts only non-mutating verbs — `hello`, `status`, `unlock`,
`list`, `search`, `get`, `generate`, `lock`. Every mutating verb is rejected at
the protocol layer with a typed error, and that rejection is a test, not a
convention.

## Alternatives Considered

### Alternative 1: Passphrase typed into the extension popup, sent over stdio

- **Pros**: no TTY needed, no platform-specific code, and saving a login works
  the way people expect from a password manager.
- **Cons**: the passphrase transits the browser process. A compromised extension,
  a malicious extension with debugger permissions, or a page that escapes world
  isolation could observe it.
- **Why not**: it trades the strongest guarantee nopass currently makes for a
  convenience feature. If the passphrase is safe to type into a browser, the
  CLI's every-mutation prompt was never buying anything.

### Alternative 2: The host spawns a native OS prompt

- **Pros**: keeps the passphrase out of the browser entirely, and keeps the
  invariant intact. `osascript` on macOS, `pinentry` or
  `systemd-ask-password` on Linux.
- **Cons**: platform-specific prompt code and a `pinentry` dependency. Worse, a
  system dialog that appears with no visible link to the browser action that
  triggered it is itself a phishing shape — users learn to approve it.
- **Why not now**: it is the right long-term answer, but it needs a designed
  request-attribution UX before it is safe. Revisit in v2.

## Consequences

### Positive

- No new authentication surface in v1. The threat model is unchanged from the
  CLI's.
- A compromised extension cannot damage or delete a store; the worst case is
  disclosure of entries the user had already unlocked.
- The security tests are plain assertions over the protocol, so a future
  contributor cannot quietly add a write path.

### Negative

- "Save this password?" — the single most expected feature of a 1Password-style
  extension — is absent. New logins are created with `nopass insert`.
- `generate` produces a password into the page and clipboard but cannot store
  it, so the user must complete the save in a terminal.

### Risks

- The gap is annoying enough that someone will be tempted to add writes without
  revisiting this decision. Mitigated by the protocol-level rejection test and
  by this ADR being the thing that must be superseded first.
