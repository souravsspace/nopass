# Releasing nopass

How to ship a new version, end to end.

crates.io, the prebuilt binaries, the `.deb`s, the nix flake and the Homebrew
tap are live. What each channel costs and what is only written down is in
[packaging/README.md](packaging/README.md).

## 1. Cut the release

```sh
# bump the version (workspace-wide, both crates inherit it)
#   Cargo.toml -> [workspace.package] version = "0.2.1"
#   crates/nopass-cli/Cargo.toml -> nopass-core = { version = "0.2.1", ... }
cargo build            # refreshes Cargo.lock
cargo test --workspace && cargo clippy --workspace --all-targets

git add Cargo.toml Cargo.lock
git commit -m "Release v0.2.1"
git tag v0.2.1
git push origin main --tags
```

The tag push starts [`release.yml`](.github/workflows/release.yml), which
tests and builds four targets, builds both `.deb`s, creates the release if
the tag beat it there, and uploads everything with a `checksums.txt`. To
fill in a release that already exists:

```sh
gh workflow run release.yml -f tag=v0.2.1
gh run watch                              # ~10 minutes
```

If the workflow cannot run, create the release by hand — `nopass update`
looks for it:

```sh
gh release create v0.2.1 --title "v0.2.1" --generate-notes
```

## 2. Publish on crates.io

Dependency order matters: `nopass-cli` depends on `nopass-core`, and
crates.io will not accept the dependent until the dependency is indexed.

```sh
cargo publish --package nopass-core
cargo publish --package nopass-cli
```

Credentials live in `~/.cargo/credentials.toml`. This is what makes both
`cargo install nopass-cli` and `cargo binstall nopass-cli` work.

## 3. Publish on Homebrew (your own tap)

One-time setup: create a repo named `homebrew-tap` under your GitHub
account, with a `Formula/` directory. Copy
[`packaging/homebrew/nopass.rb`](packaging/homebrew/nopass.rb) into it as
`Formula/nopass.rb`.

For every release, update two lines in the formula:

```sh
# get the new tarball checksum
curl -fsSL https://github.com/souravsspace/nopass/archive/refs/tags/v0.2.1.tar.gz | shasum -a 256
```

- `url` — point at the new tag's tarball
- `sha256` — the checksum you just computed

Commit and push the tap. Verify locally:

```sh
brew install --build-from-source souravsspace/tap/nopass
nopass --version
```

Users then install with:

```sh
brew tap souravsspace/tap
brew install nopass
```

## 4. The other channels

The files under [`packaging/`](packaging/) all carry the version, so bump
each one the same way — the checksums differ per channel:

| File | What to change |
|---|---|
| `packaging/aur/PKGBUILD`, `aur/.SRCINFO` | `pkgver`, tarball `sha256sums`; the `.SRCINFO` is generated with `makepkg --printsrcinfo` |
| `packaging/rpm/nopass.spec` | `Version:` |
| `packaging/nix/nopass-release.nix` | `version`, `src.hash`, `cargoHash` |

The two nix hashes are **not** the tarball checksum — `fetchFromGitHub`
hashes the unpacked tree, and `cargoHash` covers the vendored dependencies.
Get both by setting them to `lib.fakeHash`, building, and copying the value
the mismatch error prints:

```sh
nix build --impure --expr '(builtins.getFlake "git+file://'$PWD'?dirty=1").inputs.nixpkgs.legacyPackages.aarch64-darwin.callPackage '$PWD'/packaging/nix/nopass-release.nix {}'
```

`packaging/nix/nopass.nix` — the one the flake uses — builds from the
working tree and needs no hashes, so it follows the default branch on its
own.

Publishing to the AUR, COPR and nixpkgs each needs an account
somewhere; the per-channel instructions are in
[packaging/README.md](packaging/README.md).

## 5. How users update

| Installed via | Update command |
|---|---|
| anything | `nopass update` (auto-detects brew vs cargo) |
| Homebrew | `brew upgrade nopass` |
| crates.io | `cargo install nopass-cli --force` |
| cargo (git) | `cargo install --git https://github.com/souravsspace/nopass --tag v0.2.1 nopass-cli --force` |

`nopass update` queries the latest GitHub release, compares it with the
running version, and runs the right installer. `nopass update --check`
only reports whether an update exists.

## Checklist

- [ ] version bumped in `Cargo.toml` and the cli's `nopass-core` dependency,
      `Cargo.lock` refreshed (`cargo build`)
- [ ] tests + clippy green
- [ ] tag `vX.Y.Z` pushed
- [ ] release workflow green; binaries, `.deb`s and `checksums.txt` attached
- [ ] GitHub release exists (required for `nopass update`)
- [ ] `cargo publish` — core, then cli
- [ ] tap formula url + sha256 updated
- [ ] `brew install --build-from-source souravsspace/tap/nopass` works
- [ ] `packaging/` version bumps committed (aur, rpm, nix)
