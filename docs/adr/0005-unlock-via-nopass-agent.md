# ADR-0005: The extension unlocks through the existing nopass agent

**Date**: 2026-08-08
**Status**: accepted
**Deciders**: repository owner

## Context

An autofill extension that asks for a passphrase on every fill is not an
autofill extension. Some unlocked window has to exist. nopass already has one:
`nopass agent` holds unlocked identities in memory, keyed by a SHA-256 digest of
the `LockedIdentity` exactly as it sits on disk, for a TTL that defaults to zero
— caching is opt-in.

The question is whether the browser bridge reuses that window or opens a second
one of its own.

## Decision

Reuse it. `nopass-host` reads through `agent::get` and writes through
`agent::put` with the configured TTL, so terminal and browser share one unlock
state: unlocking in a terminal makes the extension work, and `nopass lock`
locks the extension too. The extension adds only its own idle timeout for the
UI and tells the host to drop its handle when the browser shuts down.

To make that possible, `agent.rs` moves from `nopass-cli` — a binary-only crate
the host cannot depend on — into `nopass-core`. `spawn()` can no longer assume
`current_exe()` is `nopass`, so it resolves the binary from `NOPASS_BIN`, then
`current_exe()` when it is named `nopass`, then `PATH`.

## Alternatives Considered

### Alternative 1: A session owned by the host, independent of the agent

- **Pros**: no change to `nopass-core`; browser-specific policy (shorter TTL,
  lock on browser close) is expressible without touching the CLI's behaviour.
- **Cons**: a second in-memory secret cache to audit, and two unlock states that
  disagree — `nopass lock` in a terminal would leave the browser unlocked, which
  is precisely the moment a user expects locking to work.
- **Why not**: two caches in a password manager is one cache too many.

### Alternative 2: Require a passphrase for every fill

- **Pros**: strictly the safest, and consistent with nopass's mutation policy.
- **Cons**: unusable. Nobody types a passphrase per login field.
- **Why not**: it would make the extension pointless.

## Consequences

### Positive

- One unlock state, one TTL setting, one place to audit. `nopass lock` means
  what it says everywhere.
- Users who already run a warm agent get a working extension with no browser
  unlock step at all.
- With the default TTL of zero, nothing changes for users who never opted into
  caching — the extension will simply ask each time.

### Negative

- `agent.rs` moves between crates, so blame on that file gets a discontinuity
  and `nopass-cli` gains a re-export.
- The extension's idle timeout and the agent's TTL are two timers that can
  disagree; the UI has to treat the agent as the authority and re-check rather
  than trust its own countdown.

### Risks

- **This is the load-bearing security trade of the whole design.** When the
  cache is cold, the passphrase is typed into the extension popup and sent to
  the host over stdio, which means it exists inside the browser process. A
  compromised browser or extension sees it, and from there sees everything —
  including the ability to run the CLI. ADR-0002 refuses this exposure for
  writes; accepting it for unlock is not a smaller version of the same risk, it
  is the same risk, and it is accepted only because there is no unlock UX
  without it in v1.
  - Mitigated as far as it can be: the passphrase is zeroized the moment it is
    sent, never written to extension storage, never crosses into a content
    script, and is exchanged once for a session rather than held.
  - The real fix is an out-of-band prompt owned by the host (ADR-0002,
    Alternative 2). Users who want that guarantee today can warm the agent from
    a terminal and never type a passphrase into the browser at all.
