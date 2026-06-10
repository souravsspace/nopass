# nopass

A fast, self-contained password manager written in Rust. Each entry is an
individually encrypted file in a simple directory tree, so your store is
trivially syncable, diffable, and git-friendly — with encryption built
straight into the CLI. No external key tooling required.

- **Built-in crypto** — modern X25519 + ChaCha20-Poly1305 (the age format),
  fully in-process. `nopass init` generates your keypair automatically.
- **Auto git sync** — every change is committed, pulled, and pushed to your
  remote automatically. Set a remote once, never think about it again.
- **Per-folder recipients** — a `.nopass-id` file in any subfolder encrypts
  that subtree to different keys (great for shared/team folders).
- **Optional GPG backend** — already have GPG keys? `NOPASS_BACKEND=gpg`
  uses them instead.

## Monorepo layout

- [`crates/nopass-core`](crates/nopass-core) — Rust library with all store
  logic (the shared engine for every frontend)
- [`crates/nopass-cli`](crates/nopass-cli) — the `nopass` CLI binary
- `apps/web` — marketing site *(planned)*
- `apps/desktop` — desktop app, Tauri over nopass-core *(planned)*
- `apps/mobile` — mobile app *(planned)*

## Installation

Requires [Rust](https://rustup.rs). Optionally `git` (for history/sync)
and `gnupg` (only for the gpg backend).

```sh
git clone https://github.com/souravsspace/nopass.git
cd nopass
cargo install --path crates/nopass-cli
nopass --version
```

Or for development, [devbox](https://www.jetify.com/devbox) provides the
whole toolchain:

```sh
devbox shell
cargo build --release   # binary at target/release/nopass
```

## Getting started, A to Z

### 1. Create your key and store

```sh
nopass init
```

That's it. This generates an encryption keypair (if you don't have one),
prints your public key, and creates the store at `~/.nopass`. Your secret
key lives at `~/.config/nopass/identity.txt`.

> **Back up `~/.config/nopass/identity.txt` somewhere safe.** Anyone with
> this file can read your passwords; without it, nobody can — including you.

You can also generate the key explicitly first:

```sh
nopass keygen            # prints your public key (age1...)
nopass keygen --force    # replace existing key (old entries become unreadable!)
```

### 2. Turn on history and sync (recommended)

```sh
nopass git init
nopass git remote add origin git@github.com:you/passwords.git
```

From now on **every change auto-commits, pulls, and pushes**. Add a
password on your laptop, it's on the remote before the prompt returns.
Other machines pick it up on their next change, or manually:

```sh
nopass git pull
```

Offline? No problem — changes commit locally and nopass prints a warning;
push later with `nopass git push`. Any git command works through the
passthrough: `nopass git log`, `nopass git status`, ...

To opt out of auto-sync: `NOPASS_AUTOSYNC=0` in your shell, or per-store
`nopass git config nopass.autosync false`.

### 3. Add passwords

```sh
nopass generate web/github          # random 25-char password, stored + printed
nopass generate web/github 40       # custom length
nopass generate -n wifi/home 16     # alphanumeric only (no symbols)
nopass generate -c web/github       # straight to clipboard, never shown
nopass insert mail/proton           # type it yourself (hidden, asked twice)
nopass insert -m notes/recovery     # multiline: paste, then Ctrl+D
```

Names are paths — `web/github`, `work/aws/root`, anything. Directories are
created on demand and cleaned up when emptied.

An entry is just lines of text. Convention: password on line 1, anything
else below:

```
hunter2
user: alice
url: https://github.com/login
```

### 4. Read passwords

```sh
nopass                       # tree of the whole store
nopass web                   # tree of one folder
nopass show web/github       # print entry
nopass web/github            # same (show is the default)
nopass show -c web/github    # copy line 1 to clipboard, auto-clear in 45s
nopass show -c2 web/github   # copy line 2 instead
```

### 5. Search

```sh
nopass find github           # match entry names
nopass grep alice            # search inside decrypted contents
```

### 6. Change things

```sh
nopass edit web/github               # open in $EDITOR
nopass generate -i web/github        # new password, keep extra lines
nopass mv web/github work/github     # move/rename (re-encrypts if needed)
nopass cp web/github web/github-bak  # copy
nopass rm web/github                 # delete (asks first; -f to skip)
nopass rm -rf old-folder             # delete a whole folder
```

### 7. Set up a second machine

```sh
# on the new machine
cargo install --path crates/nopass-cli
mkdir -p ~/.config/nopass
# copy identity.txt from your first machine (or a backup) into ~/.config/nopass/
git clone git@github.com:you/passwords.git ~/.nopass
nopass            # works — same key, same store
```

Prefer a separate key per machine? Run `nopass keygen` on the new machine,
then on any machine that has access add both public keys:

```sh
nopass init age1laptopkey... age1desktopkey...
```

This re-encrypts the store so **both** machines can decrypt it.

### 8. Share a folder with someone

Each folder can have its own recipients. Put your teammate's public key
(they get it from `nopass keygen`) alongside yours on a subfolder:

```sh
nopass init -p team/shared age1yourkey... age1theirkey...
```

Everything under `team/shared` is now encrypted to both of you; the rest
of your store stays yours alone. Moving an entry in or out of the folder
re-encrypts it automatically.

## Command reference

```
nopass keygen [--force]                  generate this machine's keypair
nopass init [-p subfolder] [recipients]  initialize store (auto-keygen if needed)
nopass [ls] [subfolder]                  list entries as a tree
nopass [show] [-c[line]] name            decrypt and print (or copy to clipboard)
nopass find terms...                     list entries matching terms
nopass grep pattern                      search decrypted contents
nopass insert [-e|-m] [-f] name          add an entry (echo / multiline / force)
nopass edit name                         edit with $EDITOR
nopass generate [-n] [-c] [-i|-f] name [length]
                                         generate password (no-symbols / clip /
                                         in-place / force)
nopass rm [-r] [-f] name                 remove entry or directory
nopass mv [-f] old new                   move + re-encrypt
nopass cp [-f] old new                   copy + re-encrypt
nopass git <args>...                     run any git command in the store
```

Aliases: `ls`=`list`, `rm`=`remove`/`delete`, `mv`=`rename`, `cp`=`copy`.

## Configuration

All optional, via environment variables:

| Variable | Default | Purpose |
|---|---|---|
| `NOPASS_DIR` | `~/.nopass` | store location |
| `NOPASS_IDENTITY` | `~/.config/nopass/identity.txt` | secret key file |
| `NOPASS_BACKEND` | `native` | `native`, `gpg`, or `plain` (tests only) |
| `NOPASS_AUTOSYNC` | `1` | `0` disables auto pull/push to the remote |
| `NOPASS_KEY` | — | override recipients for all encryption |
| `NOPASS_GENERATED_LENGTH` | `25` | default generated password length |
| `NOPASS_CHARACTER_SET` | alnum + punct | generation charset |
| `NOPASS_CHARACTER_SET_NO_SYMBOLS` | alnum | charset for `generate -n` |
| `NOPASS_CLIP_TIME` | `45` | seconds before clipboard clears |
| `NOPASS_GPG_OPTS` | — | extra flags for the gpg backend |

`EDITOR` picks the editor for `nopass edit` (default `vi`). Clipboard uses
`pbcopy` on macOS, `wl-copy` on Wayland, `xclip` on X11.

## How it works

```
~/.nopass/
├── .nopass-id          # recipients (public keys) for the store
├── web/
│   ├── github.np       # one encrypted file per entry
│   └── gitlab.np
└── team/shared/
    ├── .nopass-id      # different recipients just for this folder
    └── wifi.np
```

- Every `.np` file is encrypted with the age format (X25519 key agreement,
  ChaCha20-Poly1305 authenticated encryption) to all recipients listed in
  the nearest `.nopass-id` up the tree.
- Decryption uses your secret identity file; it never leaves your machine.
- If the store is a git repo, every mutation becomes a commit, then nopass
  pulls (rebase) and pushes — so multiple machines converge automatically.
- Generated passwords come from the OS cryptographic RNG.

## Security notes

- Entry **names are not encrypted** (they're file names). Don't put secrets
  in entry names.
- The clipboard is cleared after `NOPASS_CLIP_TIME` seconds, but other apps
  may read the clipboard during that window.
- `nopass edit` writes plaintext to a temp dir (`/dev/shm` ramdisk when
  available) for the duration of the edit.
- `keygen --force` orphans anything encrypted only to the old key. Add the
  new key as a recipient and re-init instead if you want a rotation:
  `nopass init age1newkey...` re-encrypts everything.

## Development

```sh
devbox shell                          # rust + git (+ gnupg for gpg backend)
cargo test --workspace                # 52 tests, fully hermetic
cargo clippy --workspace --all-targets
cargo fmt --all
```

Tests never touch your real store, keys, or network: store mechanics run
against a plaintext test backend, native-crypto tests use throwaway
identities in temp dirs, and sync tests push to local bare repos.

## License

MIT.
