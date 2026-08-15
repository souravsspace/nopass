# ADR-0008: An entry says what it is, on a `type:` line

**Date**: 2026-08-15
**Status**: accepted
**Deciders**: repository owner

## Context

nopass stores a password and whatever the user wanted to keep beside it: the
first line is the secret, everything after is `key: value`, and anything
unrecognised is ignored. That shape came from `pass` and has been enough for
logins.

It is not enough for what a browser is asked to fill. A sign-up form wants a
name, an address, a country, a date of birth. A checkout wants a card number,
an expiry and a security code. Those are not logins with extra lines: a card
has no username, an identity has no secret at all, and a row for a card must
never read as its number.

The store cannot be re-shaped to accommodate that. Every entry already on
disk was written by an older nopass or by hand, and a format change that
orphaned them would be worse than the gap.

## Decision

Add one line. `type: card` — or `identity`, or `passkey` — names what an
entry holds; an entry with no `type:` line is a **login**, which is what every
entry written before this decision is.

Each kind claims a set of keys, listed in `nopass-core::fields`, and each key
names the spellings it is recognised by: someone who wrote `email:` where the
list says `username:` meant the same thing. Parsing moves into
`nopass-core::record`, so the CLI and the native host read one body by one set
of rules.

Two properties make it safe to rewrite an entry this way:

- **Nothing is dropped.** A line no build understands survives a parse and a
  render in its original position, so an entry edited by an older popup keeps
  the field a newer nopass wrote.
- **A field is written under the spelling already there.** Setting `username`
  on an entry that says `email:` rewrites that line rather than adding a
  second one for the reader to choose between.

## Alternatives Considered

### Alternative 1: a JSON body for the new kinds

- **Pros**: unambiguous, nests, needs no alias table.
- **Cons**: `nopass edit` stops being a text edit, `nopass grep` stops
  matching, and an entry becomes unreadable to every older build.
- **Why not**: the format's whole value is that it is a file a person can
  read, edit and diff.

### Alternative 2: a folder per kind — `cards/`, `identities/`

- **Pros**: no format change at all.
- **Why not**: the kind would live in the path, so moving a file would change
  what it is. Folders already mean recipients, and overloading them with type
  makes `.nopass-id` inheritance mean two things.

### Alternative 3: a sidecar index of kinds

- **Why not**: a second file to keep in step with the first, and a merge
  conflict waiting for anyone who syncs a store across two machines.

## Consequences

### Positive

- Cards, identities and passkeys are ordinary entries: encrypted the same
  way, synced the same way, greppable, and editable in `$EDITOR`.
- Older builds still read the store. They see an unfamiliar `type:` line and
  ignore it, which is what they already did with any key they did not know.
- The wire speaks one canonical key per field, resolved by the host, so the
  extension needs no alias table of its own.

### Negative

- Two vocabularies to keep in step: the alias lists in `nopass-core::fields`
  and the canonical keys the wire and the popup use.
- An entry can lie about itself. A `type: card` line on something that is not
  a card produces a nonsense row, not a security problem.

### Risks

- Alias lists grow by accretion. The rule that keeps them honest: a spelling
  goes in only when a real store or a real form uses it, and the canonical key
  is always among its own aliases — there is a test for that.
