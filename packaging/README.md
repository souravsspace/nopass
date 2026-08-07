# Packaging

Every way nopass can reach a user, what each one costs to run, and what is
already live. Per-release steps are in [RELEASING.md](../RELEASING.md).

nopass is a **Unix program**: it uses unix sockets for the passphrase agent,
`stty` for hidden prompts, and `/dev/shm` when it is there. Linux and macOS
only — see [Windows](#windows) at the bottom.

## Live

| Channel | Users install with | Per release |
|---|---|---|
| **crates.io** | `cargo install nopass-cli` | `cargo publish` — core first, then cli |
| **Prebuilt binaries** | download from the release, or `cargo binstall nopass-cli` | nothing; [`release.yml`](../.github/workflows/release.yml) builds them when the tag lands |
| **Debian / Ubuntu** | `apt install ./nopass-cli_0.2.1-1_amd64.deb` from the release | nothing; the same workflow attaches both `.deb`s |
| **Nix / NixOS** | `nix profile install github:souravsspace/nopass` | nothing; the flake follows the default branch |
| **Homebrew tap** | `brew tap souravsspace/tap && brew install nopass` | bump `url` + `sha256` in [`homebrew/nopass.rb`](homebrew/nopass.rb), push the tap |
| **cargo (git)** | `cargo install --git https://github.com/souravsspace/nopass nopass-cli` | nothing; the tag is enough |
| **GitHub release** | download the source tarball | `gh release create` |

## What a tag triggers

Pushing a `v*` tag starts two workflows, and both refuse a tag that is not
an ancestor of `main`:

- [`release.yml`](../.github/workflows/release.yml) — tests and builds four
  targets, builds both `.deb`s, and attaches them to the release with a
  `checksums.txt`.
- [`publish.yml`](../.github/workflows/publish.yml) — pushes the version out
  to every channel that can be automated. Each job is skipped when its
  secret is missing, so the workflow stays green until you add one.

| Job | Repository secret | How to get it |
|---|---|---|
| crates.io | `CARGO_REGISTRY_TOKEN` | `cargo login` prints it, or crates.io → Account Settings → API Tokens |
| homebrew tap | `TAP_TOKEN` | a GitHub PAT with `contents: write` on `souravsspace/homebrew-tap` |
| aur | `AUR_SSH_KEY` | the private half of the SSH key registered on your AUR account |
| copr | `COPR_CONFIG` | the whole `~/.config/copr` file from copr.fedorainfracloud.org → API |

The AUR and COPR jobs read `packaging/aur/*` and `packaging/rpm/nopass.spec`
**at the tag**, so bump those files before tagging.

## Written, not published yet

Definitions are in this directory and build today. Publishing each one needs
an account or a merge request somewhere.

| Channel | Files | Users install with | What publishing needs |
|---|---|---|---|
| **nixpkgs** (upstream) | [`nix/nopass-release.nix`](nix/nopass-release.nix) | `nix-env -iA nixpkgs.nopass` | a PR to NixOS/nixpkgs, and a `cargoHash` bump per release |
| **AUR** (Arch) | [`aur/PKGBUILD`](aur/PKGBUILD), [`aur/.SRCINFO`](aur/.SRCINFO) | `yay -S nopass` | an AUR account with an SSH key; push to `aur@aur.archlinux.org:nopass.git` |
| **Fedora / RHEL** | [`rpm/nopass.spec`](rpm/nopass.spec) | `dnf copr enable souravsspace/nopass && dnf install nopass` | a Fedora account; COPR builds and hosts it for free |

## Worth considering, nothing written

| Channel | Why you might | Why not yet |
|---|---|---|
| **homebrew-core** | `brew install nopass`, no tap | Homebrew's notability bar: ~30 forks / 30 watchers / 75 stars, or a maintainer's judgement |
| **MacPorts** | the other macOS package manager | a Portfile PR; small audience next to Homebrew |
| **openSUSE (OBS)** | the Open Build Service can build rpm *and* deb for a dozen distros from one spec | an OBS account; effectively a second CI to look after |
| **Void, Gentoo, Guix** | thorough, opinionated distros whose users notice | one template/ebuild/definition each, and each has its own review culture |
| **asdf / mise** | version managers some developers live in | needs a plugin repo of its own |

## Not recommended

- **Snap** — strict confinement fights a password manager: it needs your
  `$HOME`, your `git`, your clipboard tool, and `$EDITOR`. Classic
  confinement lifts that, and needs manual approval from Canonical.
- **Flatpak** — built for graphical apps; a sandboxed CLI is awkward to run
  and awkward to explain.
- **Docker** — a container that has to see your key and your store is a
  worse version of installing the binary.

## Windows

`scoop`, `chocolatey` and `winget` are out of reach until the CLI builds on
Windows at all. What is in the way: `crates/nopass-cli/src/agent.rs` uses
unix sockets and `setsid`, `prompt_hidden` shells out to `stty`, `edit`
prefers `/dev/shm`, and the clipboard path assumes `pbcopy`/`wl-copy`/`xclip`.
None of it is unfixable — Windows 10+ has `AF_UNIX`, and the console API
replaces `stty` — but it is a port, not a packaging job.
