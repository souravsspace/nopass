# Releasing nopass

How to ship a new version, end to end.

Homebrew is the packaging channel that is live today. Every other one —
what it costs, what it needs, and what is already written — is in
[packaging/README.md](packaging/README.md).

## 1. Cut the release

```sh
# bump the version (workspace-wide, both crates inherit it)
#   Cargo.toml -> [workspace.package] version = "0.2.0"
cargo test --workspace && cargo clippy --workspace --all-targets

git add Cargo.toml Cargo.lock
git commit -m "Release v0.2.0"
git tag v0.2.0
git push origin main --tags
```

Then create the GitHub release from the tag (this is what `nopass update`
looks for):

```sh
gh release create v0.2.0 --title "v0.2.0" --generate-notes
```

## 2. Publish on Homebrew (your own tap)

One-time setup: create a repo named `homebrew-tap` under your GitHub
account, with a `Formula/` directory. Copy
[`packaging/homebrew/nopass.rb`](packaging/homebrew/nopass.rb) into it as
`Formula/nopass.rb`.

For every release, update two lines in the formula:

```sh
# get the new tarball checksum
curl -fsSL https://github.com/souravsspace/nopass/archive/refs/tags/v0.2.0.tar.gz | shasum -a 256
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

## 3. How users update

| Installed via | Update command |
|---|---|
| anything | `nopass update` (auto-detects brew vs cargo) |
| Homebrew | `brew upgrade nopass` |
| cargo | `cargo install --git https://github.com/souravsspace/nopass --tag v0.2.0 nopass-cli --force` |

`nopass update` queries the latest GitHub release, compares it with the
running version, and runs the right installer. `nopass update --check`
only reports whether an update exists.

## Checklist

- [ ] version bumped in `Cargo.toml`, `Cargo.lock` refreshed (`cargo build`)
- [ ] tests + clippy green
- [ ] tag `vX.Y.Z` pushed
- [ ] GitHub release created (required for `nopass update`)
- [ ] tap formula url + sha256 updated
- [ ] `brew install --build-from-source souravsspace/tap/nopass` works
