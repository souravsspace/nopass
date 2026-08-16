# ADR-0010: nopass is the authenticator, and the CLI is where it starts

**Date**: 2026-08-16
**Status**: accepted
**Deciders**: repository owner

## Context

A password manager that cannot hold passkeys is a password manager with an
expiry date. Sites are moving to WebAuthn, and a passkey is not a secret a
manager can store the way it stores a password: there is nothing to type. The
manager has to *be* the authenticator — make the keypair, keep the private
half, and sign what a site asks.

Two shapes were possible. Make nopass an authenticator the browser can route
to, which on Chromium means `chrome.webAuthenticationProxy` and on Gecko means
nothing at all, because Firefox has no equivalent API. Or make nopass an
authenticator a person can drive, and wire a browser to it afterwards.

## Decision

Build the authenticator first, in `nopass-core::passkey`, and expose it as
`nopass webauthn register | list | assert | verify`.

It implements the authenticator half of WebAuthn to the byte: ES256 over
P-256, `rpIdHash ‖ flags ‖ signCount` authenticator data, attested credential
data carrying a COSE key, a CBOR attestation object, and an assertion over
`authenticatorData ‖ SHA-256(clientDataJSON)`. `register` and `assert` print
exactly the JSON a browser hands to a site, so the output can be posted to a
relying party unchanged.

Attestation is **none**, and the AAGUID is zero. Attestation tells a site what
hardware it is talking to; a software authenticator keeping its key in an
age-encrypted file has nothing to attest that a site should weigh differently.

The private key is a `type: passkey` entry (ADR-0008): the first line is the
key, and `rp`, `user`, `credential-id`, `alg` and `counter` are its fields. It
is encrypted to the same recipients, synced by the same git, and read only
after the same unlock as everything else.

`verify` is the relying party's half, run locally. Nothing needs it to sign
in — the site does that — and it exists so a signature can be watched to hold
rather than taken on trust. The tests use it as the relying party they
otherwise would not have.

## Alternatives Considered

### Alternative 1: `chrome.webAuthenticationProxy` first

- **Pros**: passkeys that work on real sites, in the browser, immediately.
- **Cons**: Chromium only; nothing is verifiable until the whole chain works;
  and it still needs everything in this ADR underneath it.
- **Why not now**: it is the next milestone, not the first one. The
  authenticator is the part with cryptography in it, and it can be proved
  correct on its own.

### Alternative 2: store a passkey exported from a browser

- **Why not**: browsers do not export them, by design. There is nothing to
  store.

### Alternative 3: a passkey crate that implements both halves

- **Pros**: less code here.
- **Why not**: the authenticator half is a few hundred lines of byte layout
  that this documents in place; the crates that cover it are relying-party
  libraries carrying a dependency tree for the half nopass is not.

## Consequences

### Positive

- A passkey is an ordinary entry: same crypto, same sync, same backups, and
  `nopass show` reads it.
- The counter is written back before the assertion is printed, so a signature
  that went out is never one the store forgot — a counter that goes backwards
  is exactly what a site reads as a cloned credential.

### Negative

- Losing the store now loses accounts rather than only passwords, because the
  site keeps the other half of the key and no password exists to fall back on.
  The README says so where it talks about backups.
- The user has to move a challenge and a response by hand until the browser
  half lands.

### Risks

- `user_verified` is asserted as true because unlocking the store is the
  verification. That is a claim to the site about what happened at this
  machine, and it is true of nopass's own gate rather than of a biometric.
- A software authenticator cannot resist a compromised account: anything that
  can read the store while it is unlocked can sign. Hardware keys exist for
  that threat, and `nopass passkey --security-key` still locks the identity
  with one.
