# Releasing nopass

How to ship a new version, end to end.

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

## 3. Signed macOS build (only needed for Touch ID)

Everything in nopass works from an ordinary `cargo install` **except** the
Touch ID slot. macOS only hands out biometry-gated Secure Enclave keys to a
binary whose signature carries an application identifier and a matching
keychain access group, and those entitlements are only valid when an embedded
provisioning profile authorizes them. An unsigned build gets
`errSecMissingEntitlement (-34018)`; a build signed with those entitlements but
no profile is killed at launch. There is no way around this from source.

One-time setup, in the [Apple Developer portal](https://developer.apple.com/account):

1. Join the Apple Developer Program (a free account can issue *development*
   profiles that work on your own registered Macs; distributing to other people
   needs the paid membership and a Developer ID certificate).
2. Register an App ID — e.g. `com.yourname.nopass` — with the **Keychain
   Sharing** capability enabled.
3. Create a provisioning profile for that App ID (Mac Development for personal
   use, Developer ID for distribution) and download it.
4. Note your 10-character team identifier.

Then build:

```sh
TEAM_ID=ABCDE12345 \
BUNDLE_ID=com.yourname.nopass \
SIGN_IDENTITY="Developer ID Application: Your Name (ABCDE12345)" \
PROFILE=~/Downloads/nopass.provisionprofile \
NOTARY_PROFILE=nopass-notary \
packaging/macos/sign.sh
```

`security find-identity -v -p codesigning` lists your identities.
`NOTARY_PROFILE` is optional and refers to credentials stored with
`xcrun notarytool store-credentials`; skip it to sign without notarizing.

The script builds `dist/nopass.app` (the CLI lives at
`Contents/MacOS/nopass`, with the profile embedded beside it), signs it,
prints the entitlements it actually got, and then **runs it** — a binary whose
entitlements the profile does not cover signs cleanly and is still killed on
launch, so signing successfully proves nothing on its own.

Attach `dist/nopass-macos.zip` to the GitHub release. Users install the app
and symlink the executable onto their PATH; the script prints the exact
commands. Homebrew users who install the from-source formula get every feature
except Touch ID.

## 4. How users update

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
- [ ] (for Touch ID) `packaging/macos/sign.sh` run, signed zip attached to the release
