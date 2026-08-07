# ADR-0004: Turborepo and Bun workspaces beside the Cargo workspace

**Date**: 2026-08-08
**Status**: accepted
**Deciders**: repository owner

## Context

The repository is a Cargo workspace with two crates. The browser bridge adds a
third crate and, for the first time, JavaScript: a protocol package, a shared
UI package, the extension source, two build targets and a workbench app. That
is two build graphs in one repository, and the protocol package and the host
crate are two halves of the same contract, so they have to be versioned and
tested together.

## Decision

Keep the Cargo workspace exactly as it is and add a Bun workspace next to it.
Root `package.json` declares `packages/*` and `tools/*`; `turbo.json` defines
the JavaScript task graph and wraps the Cargo commands as passthrough tasks, so
one `turbo run test` at the root runs both languages.

## Alternatives Considered

### Alternative 1: A separate repository for the extension

- **Pros**: clean separation, and the JavaScript tooling never touches the Rust
  release process.
- **Cons**: the wire protocol lives on both sides of the split. Every change
  becomes a two-repo dance, and a version skew between host and extension is a
  silent failure in a security product.
- **Why not**: the contract is the whole point of the coupling.

### Alternative 2: Cargo plus plain npm scripts, no Turborepo

- **Pros**: nothing new to learn, and no extra configuration file.
- **Cons**: no task graph, so nothing knows that the extension build depends on
  the protocol build, and no caching, so CI re-runs everything on every push.
- **Why not**: six packages is past the point where hand-ordered scripts stay
  correct.

### Alternative 3: Nx

- **Pros**: more capable graph, plugins for most ecosystems.
- **Cons**: substantially more configuration and a heavier mental model for a
  repository this size.
- **Why not**: Turborepo has first-class Bun workspace support and covers the
  need with one file.

## Consequences

### Positive

- `bun run test` at the root is the single entry point for contributors,
  whichever language they touched.
- The protocol package and the host crate are tested against the same JSON
  fixtures in the same CI job, so the contract cannot drift unnoticed.
- Turborepo caches the JavaScript builds; Cargo keeps caching the Rust ones.

### Negative

- Two lockfiles, two dependency-update flows, and two toolchains a contributor
  must install.
- The Rust tasks are opaque to Turborepo's cache — it can only shell to Cargo
  and let Cargo decide.

### Risks

- Release tooling currently assumes a Cargo-only repository. `RELEASING.md` and
  the tag workflow have to learn that `packages/` and `tools/` exist, or the
  extension will silently never ship.
