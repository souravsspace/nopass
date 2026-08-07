# Debian and Ubuntu

`.deb` packages are built with [cargo-deb](https://github.com/kornelski/cargo-deb),
which derives the control file from `Cargo.toml`. Nothing here has to be kept
in step with the crate by hand:

```sh
cargo install cargo-deb
packaging/debian/build-deb.sh          # → target/debian/nopass-cli_0.2.1-1_arm64.deb
sudo apt install ./target/debian/nopass-cli_*.deb
```

## Optional: pin the metadata

The defaults are fine. If you want a fuller control file — recommended
packages, a longer description, extra installed files — add this to
`crates/nopass-cli/Cargo.toml` and drop the `--maintainer` flag from the
script:

```toml
[package.metadata.deb]
maintainer = "Sourav <souravsspace@gmail.com>"
copyright = "2026, Sourav"
license-file = ["../../LICENSE", 0]
extended-description = """
nopass keeps each password in its own age-encrypted file under one directory.
Every command that reads or changes the store asks for your master passphrase;
reads can reuse a cached one when you opt in."""
depends = "$auto"
recommends = "git"
suggests = "xclip, wl-clipboard, gnupg"
section = "utils"
priority = "optional"
assets = [
    ["../../target/release/nopass", "usr/bin/", "755"],
    ["../../README.md", "usr/share/doc/nopass/README.md", "644"],
]
```

## Getting it to users

Ranked by effort:

1. **Attach the `.deb` to the GitHub release.** One `gh release upload`, works
   for both Debian and Ubuntu, no infrastructure. Users download and
   `apt install ./nopass.deb`.
2. **A Launchpad PPA** (`ppa:souravsspace/nopass`). Ubuntu only, needs a
   Launchpad account and a GPG key, and builds from a source package rather
   than a binary — Rust crates must be vendored for their builders.
3. **Your own apt repository** (aptly, or a `.deb` in a GitHub Pages repo
   signed with a release key). Users add one line to sources.list.
4. **Debian proper.** Needs a sponsor, and every crate in the dependency tree
   packaged separately as `librust-*-dev`. Months, not evenings.
