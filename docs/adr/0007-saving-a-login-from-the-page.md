# ADR-0007: A page may offer a login; only the user may name one

**Date**: 2026-08-15
**Status**: accepted
**Amends**: ADR-0006
**Deciders**: repository owner

## Context

ADR-0006 added `insert` and put it in `PopupRequest` alone: "`ContentRequest`
gains nothing." That is the right shape for the store and the wrong shape for
the moment that matters. A login is finished being typed on the page, not in
the popup, and by the time the user has opened the popup the fields are gone
with the navigation. The result is that nopass has a create verb nobody
reaches: an entry can only be added by retyping a password the browser just
watched being typed.

Every password manager answers this the same way, and the reason is not
convenience. A user who has to retype the password picks one they can retype.

## Decision

A content script may **offer** a login and may **name** one. It still may not
save one of its own composition.

Three requests, all carrying the tab id from `sender` rather than from the
message:

- `captured` — the two fields of a form that was just submitted. The
  background holds them in memory, keyed by tab, and answers with whether the
  offer is worth putting on screen at all.
- `saveCaptured` — a name, and nothing else. The password inserted is the one
  the background is already holding for that tab.
- `dismissCaptured` — throw it away.

Nothing is written without a name the user has seen and can rewrite, in a
panel in a closed shadow root the page cannot reach. `insert` is unchanged, so
the two bounds from ADR-0006 still hold: the store must be unlocked, and the
name must be free.

The offer is withheld — silently, no prompt — when the store is locked, when
the origin is not a web page, or when the site already has an entry under that
username. A prompt for a login the user just signed in with is noise.

## Alternatives Considered

### Alternative 1: prompt in the popup instead of on the page

- **Pros**: `ContentRequest` gains nothing; ADR-0006 stands unamended.
- **Cons**: the popup is not open, and an extension cannot open it in Gecko.
  The offer would be a badge the user has to notice and act on later.
- **Why not**: it moves the prompt away from the moment it is about. The whole
  problem is that the moment passes.

### Alternative 2: let the content script send `save` outright

- **Pros**: one request instead of three.
- **Why not**: it hands the write path a payload the background never saw the
  provenance of. Keeping the password on the background side means the worst a
  `saveCaptured` can do is file a credential the user typed into that page
  under a name they were shown.

## Consequences

### Positive

- Saving a login finishes where it starts. A generated password is now
  something the user never has to be able to retype.
- The password crosses the extension once, on submit, and lives in the
  background's memory until the prompt is answered or the tab closes.

### Negative

- The content script's vocabulary is three words longer, and one of them
  reaches — indirectly — the only mutating verb the host has.
- A capture heuristic will sometimes be wrong: a form that is not a sign-in
  can raise a prompt. Dismissing it writes nothing.

### Risks

- A hostile page cannot send these itself — a page script has no access to
  the isolated world, and `externally_connectable` is unset — but it can put
  values in its own fields and submit them. The result is a prompt naming the
  page's own host, which the user declines. Untrusted events are ignored, so
  a synthetic click cannot raise one at all.
- A compromised extension gains nothing here that ADR-0006 did not already
  give it: it could call `save` from the popup surface regardless. What it
  still cannot do is read, alter or delete what is in the store.
