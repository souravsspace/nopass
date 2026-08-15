# ADR-0009: The extension may rewrite an entry, field by field

**Date**: 2026-08-15
**Status**: accepted
**Amends**: ADR-0006
**Deciders**: repository owner

## Context

ADR-0006 allowed exactly one mutating verb, `insert`, and gave the reason
plainly: overwriting is what a write path would be worth attacking. Replacing
`web/bank.example` with a password the attacker knows turns a disclosure bug
into an account takeover, while creating an entry the user never uses does
nothing for anyone.

That held while the extension only stored logins. It stops holding now that it
stores what ADR-0008 added. A card gets a new expiry every three years. An
address changes when someone moves. A stored username is wrong the moment a
site is asked to change it. "Create it again under a different name" is not an
answer for any of those, and "open a terminal" is the complaint ADR-0006 was
written to answer in the first place.

The decryption argument ADR-0002 made does not apply either. Reading an entry
already decrypts it — `get` has always done that — so `update` needs no
capability the extension does not have.

## Decision

Add `update`. It carries an entry name, an optional secret, and a list of
fields, and it is bounded on four sides:

- **It cannot create.** A name that is not in the store comes back
  `not_found`. Claiming a name is still `insert`'s job, and still refused if
  the name is taken.
- **It cannot delete.** It clears a *field* when given an empty value; there
  is no shape of this request that removes an entry.
- **It rewrites only what it names.** Every other line — including a field
  written by a newer nopass, or by hand — stays exactly where it was.
- **The store must be unlocked**, as for `insert`.

In the popup, an edit is a two-step: the fields become inputs, and the save
button asks once more, naming the entry it is about to replace. The host
cannot verify that a human agreed, so this is a rule the UI keeps rather than
a property of the wire — which is why it is written down here.

`edit`, `rm`, `mv` and `cp` stay refused under their own names, and the test
that proves it is unchanged.

## Alternatives Considered

### Alternative 1: keep create-only, and add "create a replacement"

- **Pros**: ADR-0006 stands unamended.
- **Why not**: it leaves the old entry behind, so the store fills with
  `web/bank.example-2` and a later fill has to guess between them. It is the
  same overwrite with worse bookkeeping.

### Alternative 2: whole-body replace, as `nopass edit` does

- **Pros**: one obvious operation, no merge semantics to reason about.
- **Why not**: an older popup would send back a body missing every field it
  did not understand, and silently drop them. Naming fields is what makes an
  edit from any client safe.

### Alternative 3: a native OS prompt for every update (ADR-0002's answer)

- **Pros**: a warm cache would never authorise a rewrite.
- **Cons**: `osascript` / `pinentry` / `systemd-ask-password` across three
  platforms, plus an attribution UX so the dialog is not itself a phishing
  shape.
- **Why not now**: the confirmation in the popup covers the accident, and the
  passphrase it would ask for protects nothing this operation uses. It remains
  the right answer if `update` ever needs to reach beyond one entry.

## Consequences

### Positive

- A card's expiry, an address, a username can be fixed where they are noticed.
- Merge semantics mean an edit never costs a field the editing client had
  never heard of.

### Negative

- A compromised extension, while the store is unlocked, can now rewrite what
  it can already read. It still cannot delete, move, or claim a new name over
  an old one.
- "The extension never changes an entry" was a sentence the footer could
  carry. It now says what is true instead.

### Risks

- The confirmation is UI-only, so a build that skipped it would still be
  accepted by the host. The e2e test asserts the two-step, which is the only
  place that can.
