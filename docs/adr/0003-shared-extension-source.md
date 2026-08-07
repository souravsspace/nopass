# ADR-0003: One extension source, two thin browser targets

**Date**: 2026-08-08
**Status**: accepted
**Deciders**: repository owner

## Context

nopass ships a Chrome extension and a Firefox extension. Both are Manifest V3,
both talk to the same native host, and both present the same UI. The genuine
differences are small and structural: Chrome uses a service worker background
and derives its ID from a `key` in the manifest, Firefox uses an event page and
requires `browser_specific_settings.gecko.id`, and the two disagree about
whether host permissions are granted at install time.

Everything else — form detection, the inline fill dropdown, the popup, the
native port, the session state machine — is identical.

## Decision

`packages/extension` holds all logic, UI and tests. `tools/chrome` and
`tools/firefox` are thin [WXT](https://wxt.dev) configurations that set the
browser target, the pinned extension ID, and the packaging output. WXT's
`browser` target and per-entrypoint `include`/`exclude` handle the divergence.

## Alternatives Considered

### Alternative 1: Two complete, independent copies

- **Pros**: total freedom to diverge per browser; no abstraction to fight when
  one browser needs something odd.
- **Cons**: every fix has to be made twice, and drift is not a possibility but a
  certainty. A password manager with two implementations has two threat models.
- **Why not**: the divergence is a handful of manifest keys, not behaviour.

### Alternative 2: A single WXT project built with `wxt -b chrome|firefox`

- **Pros**: the most idiomatic WXT setup, and the least configuration.
- **Cons**: no directories named `chrome` and `firefox`, so per-browser
  packaging, store metadata and signing config have nowhere natural to live.
- **Why not**: the thin-target layout gets the same single source while still
  giving each browser a home for the things that really are per-browser.

## Consequences

### Positive

- One bug fix covers both browsers; one test suite covers both.
- Per-browser packaging, store listings and signing keys have an obvious home.
- `turbo run build --filter=chrome` and `--filter=firefox` are independent
  cacheable tasks.

### Negative

- Three packages instead of one, and the indirection is not free to read the
  first time.
- WXT's entrypoint discovery has to be pointed at the shared package rather
  than the target's own directory.

### Risks

- A browser-specific hack could be written into the shared package with a
  runtime `if (import.meta.env.BROWSER === ...)` rather than into the target.
  Mitigated by keeping manifest-shaped differences in the target configs and
  requiring behavioural differences to go through WXT's `include`/`exclude`.
