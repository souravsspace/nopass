# Packaging

Every way nopass can reach a user, what each one costs to run, and what is
already live. Per-release steps are in [RELEASING.md](../RELEASING.md).

nopass ships through four channels and no more: **cargo, npm, nix and
Homebrew**. Debian, the AUR and Fedora were written and then dropped — each
one wanted an account, a review queue or a second set of checksums to keep in
step, for an audience the four above already reach.

nopass is a **Unix program**: it uses unix sockets for the passphrase agent,
`stty` for hidden prompts, and `/dev/shm` when it is there. Linux and macOS
only — see [Windows](#windows) at the bottom.

## Live

| Channel | Users install with | Per release |
|---|---|---|
| **crates.io** | `cargo install nopass-cli` | nothing; the `crates` job publishes core then cli |
| **npm** | `npm install -g nopass-cli` | nothing; the `npm` job publishes [`npm/`](npm/) |
| **Nix / NixOS** | `nix profile install github:souravsspace/nopass` | nothing; the flake follows the default branch |
| **Homebrew tap** | `brew tap souravsspace/tap && brew install nopass` | nothing; the `homebrew` job rewrites the formula and pushes the tap |
| **Prebuilt binaries** | download from the release, or `cargo binstall nopass-cli` | nothing; [`release.yml`](../.github/workflows/release.yml) builds them when the tag lands |
| **cargo (git)** | `cargo install --git https://github.com/souravsspace/nopass nopass-cli` | nothing; the tag is enough |

## What a tag triggers

Pushing a `v*` tag starts two workflows. Both begin with a `guard` job that
refuses a tag which is not an ancestor of `main`, and every other job waits
on it — so a tag on a side branch publishes nothing, anywhere.

- [`release.yml`](../.github/workflows/release.yml) — tests and builds four
  targets, attaches them to the release with a `checksums.txt`, creating the
  release if the tag push beat it there.
- [`publish.yml`](../.github/workflows/publish.yml) — pushes the version out
  to crates.io, npm and the tap. Each job is skipped when its secret is
  missing, so the workflow stays green until you add one.

| Job | Repository secret | How to get it |
|---|---|---|
| crates.io | `CARGO_REGISTRY_TOKEN` | crates.io → Account Settings → API Tokens |
| npm | `NPM_TOKEN` | npmjs.com → Access Tokens → Granular, write access to `nopass-cli` |
| homebrew tap | `TAP_TOKEN` | a GitHub PAT with `contents: write` on `souravsspace/homebrew-tap` |

The npm job waits for `release.yml` to attach the binaries before publishing:
the package downloads one on install, so shipping it early would ship
something that cannot install.

## Written, not published yet

| Channel | Files | Users install with | What publishing needs |
|---|---|---|---|
| **nixpkgs** (upstream) | [`nix/nopass-release.nix`](nix/nopass-release.nix) | `nix-env -iA nixpkgs.nopass` | [PR #550299](https://github.com/NixOS/nixpkgs/pull/550299) to be merged; after that their update bot handles versions |

## Worth considering, nothing written

| Channel | Why you might | Why not yet |
|---|---|---|
| **homebrew-core** | `brew install nopass`, no tap | Homebrew's notability bar: ~30 forks / 30 watchers / 75 stars, or a maintainer's judgement |
| **MacPorts** | the other macOS package manager | a Portfile PR; small audience next to Homebrew |
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
