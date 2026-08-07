# ADR-0001: Native messaging over a localhost server

**Date**: 2026-08-08
**Status**: accepted
**Deciders**: repository owner

## Context

The nopass store is a tree of age-encrypted files on disk. A browser extension
runs in a sandbox with no filesystem and no process access, so to autofill a
login it has to reach a local process that can decrypt. Two patterns are
established for that: **native messaging**, where the browser itself launches a
host binary and exchanges JSON over its stdin and stdout, and a **localhost
HTTP or WebSocket server** the extension connects to over TCP.

nopass already makes this choice once, for its passphrase cache: `nopass agent`
listens on a unix socket inside a directory only its owner can reach, precisely
so that no port and no network authentication are involved. The browser bridge
should not undo that.

## Decision

Use native messaging. A new binary crate, `crates/nopass-host`, is launched by
the browser and speaks `uint32`-little-endian length-prefixed JSON on stdio. It
links `nopass-core` directly.

## Alternatives Considered

### Alternative 1: Localhost HTTP or WebSocket server

- **Pros**: trivially debuggable with curl; one server serves many clients,
  including a future web app; no per-browser install step.
- **Cons**: an open TCP port that any local process — and, via `fetch` to
  `127.0.0.1`, any web page the user visits — can probe. Needs its own
  authentication, its own origin pinning, and its own lifetime management,
  because it outlives the browser.
- **Why not**: it converts a filesystem-permission problem into a network
  authentication problem. Native messaging gets the same result with the
  operating system doing the access control: the browser will only launch the
  host for the extension IDs named in the host's manifest.

### Alternative 2: Reimplement age decryption in WASM inside the extension

- **Pros**: no native component and no install step at all.
- **Cons**: the store and the identity would both have to be readable by the
  extension, which means long-lived key material sitting in browser storage.
  Passkey and FIDO2 slots are unreachable from a page context.
- **Why not**: extension-held key material is the exact thing native messaging
  exists to avoid. It would make the browser — the most exposed process on the
  machine — the custodian of the vault.

### Alternative 3: A thin host that shells out to the `nopass` CLI

- **Pros**: zero duplicated logic; the CLI stays the single implementation.
- **Cons**: a process spawn per request, secrets crossing an argv and stdout
  boundary, and a dependency on human-formatted output that is not a stable
  interface.
- **Why not**: `nopass-core` is already a library. The host links it.

## Consequences

### Positive

- No listening port exists at any point.
- The browser pins the host to specific extension IDs through `allowed_origins`
  (Chrome) and `allowed_extensions` (Firefox); the OS enforces it.
- The extension never holds an identity. A decrypted secret travels host →
  background → content script for one fill and is cleared after use.
- Crypto has one implementation, in `nopass-core`, shared by CLI and host.

### Negative

- There is an install step: a native messaging manifest has to be written to a
  browser-specific and OS-specific path. `nopass-host install` does this.
- Extension IDs must be pinned at build time — a `key` in the Chrome manifest,
  `browser_specific_settings.gecko.id` in Firefox — because the host manifest
  names them.
- Unix only, matching `nopass-core`'s use of `std::os::unix`. Windows is out of
  scope.

### Risks

- A mis-scoped `allowed_origins` would let any installed extension talk to the
  host. Mitigated by generating the manifest from the built extension's own ID
  in `nopass-host install`, rather than asking anyone to hand-edit it.
- Native messaging has no message framing beyond the length prefix, so a
  malformed length would let a peer ask the host to allocate freely. The host
  caps message size, the same way `nopass agent` caps `MAX_REQUEST`.
