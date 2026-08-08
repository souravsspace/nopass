# ADR-0006: The extension may create an entry, and only create one

**Date**: 2026-08-09
**Status**: accepted
**Supersedes**: ADR-0002, in one respect only
**Deciders**: repository owner

## Context

ADR-0002 made the browser host read-only and predicted its own successor: "the
gap is annoying enough that someone will be tempted to add writes without
revisiting this decision." This is that revisit.

It rejected writing for one reason: nopass asks the user to authenticate for
every mutation, a native messaging host has no controlling terminal to run that
prompt on, and the alternatives all moved the master passphrase into the
browser process.

**That reasoning does not hold for creating an entry.** nopass encrypts with
age, which is public-key. `Store::insert` calls
`crypto.encrypt(contents, &recipients, &file)` — recipients being the public
keys in the store's `.nopass-ids`. No identity is read, no passphrase is
needed, nothing is decrypted. The prompt in `cmd_insert` is
`store.authenticate()`, a **program check** that the person at the keyboard is
the owner; the code comment in `agent.rs` has always been explicit that it is
"not cryptography — anything running as you can still encrypt to the public
key".

So the question is not "may the passphrase enter the browser" — it need not.
The question is narrower: **may the browser add to the store?**

## Decision

Add exactly one mutating verb, `insert`, carrying `entry`, `password`, and
optionally `username` and `url`. It is allowed only when:

- the **store is unlocked** — a human typed the passphrase on this machine
  inside the current lease; and
- the **name is free** — an existing entry is answered with a new `exists`
  error code and nothing is written.

`edit`, `rm`, `mv`, `cp` and `init` stay rejected at the protocol layer, with
the same typed refusal and the same test. There is still no shape of a request
this host accepts that reaches an entry already in the store.

Fields are rejected if they contain a line break. An entry body is
`password\nkey: value\n…`, so a value free to carry a newline could forge a
`url:` line — a phishing primitive rather than a formatting bug — and it is
refused on both sides of the wire.

`ContentRequest` gains nothing. A page's content script still cannot ask for a
secret and now still cannot write one; `save` exists only in `PopupRequest`.

## Alternatives Considered

### Alternative 1: keep the store read-only, compose a `nopass insert` line

- **Pros**: no new attack surface whatsoever.
- **Why not**: it is what ADR-0002 already left users with, and the complaint
  is precisely that saving finishes in a terminal.

### Alternative 2: a native OS prompt for every write (ADR-0002's own successor)

- **Pros**: preserves "a warm cache never authorises a write" exactly.
- **Cons**: `osascript` / `pinentry` / `systemd-ask-password` on three
  platforms, plus an attribution UX so a dialog with no visible link to the
  browser action is not itself a phishing shape.
- **Why not now**: it buys nothing here. The prompt would protect a passphrase
  that this operation never uses. It becomes the right answer again the moment
  a verb needs to *decrypt* something to do its job — which is exactly what
  `edit` and `rm` would need, and why they are still refused.

### Alternative 3: allow overwrite, since the CLI does

- **Why not**: overwriting is the whole reason a write path would be worth
  attacking. Replacing `web/bank.example` with a password the attacker knows
  turns a disclosure bug into an account takeover. Creating a new entry the
  user never uses does nothing for an attacker.

## Consequences

### Positive

- "Save this login" works, with the name, username, password and the `url:`
  line that makes a later fill match.
- The master passphrase still never enters the browser process. The threat
  model for **confidentiality** is unchanged from ADR-0002.

### Negative

- A compromised extension can add entries to the store. It cannot read more
  than it already could, cannot alter what is there and cannot delete anything,
  but it can fill the store with junk the user has to clean up in a terminal.
- "The extension never writes" was a simple sentence and is now a longer one.
  The popup footer and the README say the longer one rather than eliding it.

### Risks

- The next request will be an edit button, and it does not follow from this:
  editing decrypts, which puts us back in ADR-0002's problem, where
  Alternative 2 above is the answer. Superseding this ADR is the first step,
  as it was for that one.
