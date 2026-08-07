# Releasing nopass

Releasing is a tag push. CI does the rest.

nopass ships through four channels — cargo, npm, nix and Homebrew. What each
one costs is in [packaging/README.md](packaging/README.md).

## Cut the release

```sh
# bump the version (workspace-wide, both crates inherit it)
#   Cargo.toml -> [workspace.package] version = "0.2.2"
#   crates/nopass-cli/Cargo.toml -> nopass-core = { version = "0.2.2", ... }
cargo build            # refreshes Cargo.lock
cargo test --workspace && cargo clippy --workspace --all-targets

git add Cargo.toml Cargo.lock
git commit -m "Release v0.2.2"
git push origin main
git tag v0.2.2
git push origin v0.2.2
```

Push the commit to `main` **before** the tag. Both workflows start with a
`guard` job that refuses a tag which is not an ancestor of `origin/main`, so
a tag that got there first fails the guard and publishes nothing.

## What happens then

[`release.yml`](.github/workflows/release.yml) tests and builds four targets,
creates the release if the tag push beat it there, and attaches the binaries
with a `checksums.txt`.

[`publish.yml`](.github/workflows/publish.yml) publishes both crates to
crates.io in dependency order, waits for those binaries and then publishes
`nopass-cli` to npm, and rewrites the Homebrew formula with the new tarball
checksum and pushes it to the tap.

Both are idempotent: an already-published version is skipped, not retried.
Watch them with:

```sh
gh run watch                                # ~10 minutes
```

To fill in a release that already exists, or to re-run a channel that was
missing its secret at the time:

```sh
gh workflow run release.yml -f tag=v0.2.2
gh workflow run publish.yml -f tag=v0.2.2
```

## The one thing CI cannot do

`packaging/nix/nopass-release.nix` — the file the nixpkgs package is copied
from — carries two hashes that only a nix build can compute:

| Field | What it covers |
|---|---|
| `src.hash` | the unpacked source tree |
| `cargoHash` | the vendored dependencies; changes with `Cargo.lock` |

Neither is the tarball checksum, because `fetchFromGitHub` hashes the
unpacked tree. Get both by setting them to `lib.fakeHash`, building, and
copying the value the mismatch error prints:

```sh
nix build --impure --expr '(builtins.getFlake "git+file://'$PWD'?dirty=1").inputs.nixpkgs.legacyPackages.aarch64-darwin.callPackage '$PWD'/packaging/nix/nopass-release.nix {}'
```

That build also runs all 162 tests in the sandbox, so it is worth doing even
when the hashes have not changed.

`packaging/nix/nopass.nix` — the one the flake uses — builds from the working
tree and needs no hashes, so `nix profile install github:souravsspace/nopass`
follows `main` on its own.

## Secrets the workflows need

A job whose secret is missing is skipped, not failed. Set them once:

```sh
gh secret set CARGO_REGISTRY_TOKEN     # crates.io → Account Settings → API Tokens
gh secret set NPM_TOKEN                # npmjs.com → Access Tokens → Granular
gh secret set TAP_TOKEN                # GitHub PAT, contents:write on the tap repo
```

## How users update

| Installed via | Update command |
|---|---|
| anything | `nopass update` (auto-detects brew vs cargo) |
| Homebrew | `brew upgrade nopass` |
| crates.io | `cargo install nopass-cli --force` |
| npm | `npm install -g nopass-cli@latest` |
| nix | `nix profile upgrade nopass` |

`nopass update` queries the latest GitHub release, compares it with the
running version, and runs the right installer. `nopass update --check` only
reports whether an update exists.

## Checklist

- [ ] version bumped in `Cargo.toml` and the cli's `nopass-core` dependency,
      `Cargo.lock` refreshed (`cargo build`)
- [ ] tests + clippy green
- [ ] `packaging/nix/nopass-release.nix` version + both hashes bumped, and
      the build above is green
- [ ] commit on `main`, **then** the tag
- [ ] both workflows green
- [ ] release has four binaries and a `checksums.txt`
- [ ] `cargo install nopass-cli`, `npm install -g nopass-cli` and
      `brew upgrade nopass` all land on the new version
