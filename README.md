# nopass

A fast, self-contained password manager written in Rust. Each entry is an
individually encrypted file in a simple directory tree, so your store is
trivially syncable, diffable, and git-friendly — with encryption built
straight into the CLI. No external key tooling required.

- **Built-in crypto** — modern X25519 + ChaCha20-Poly1305 (the age format),
  fully in-process. `nopass init` generates your keypair automatically.
- **Per-folder recipients** — a `.nopass-id` file in any subfolder encrypts
  that subtree to different keys (great for shared/team folders).
- **Automatic git history** — every change is committed if the store is a
  git repo.
- **Optional GPG backend** — already have GPG keys? `NOPASS_BACKEND=gpg`
  uses them instead.

## Monorepo layout

- [`crates/nopass-core`](crates/nopass-core) — Rust library with all store
  logic (the shared engine for every frontend)
- [`crates/nopass-cli`](crates/nopass-cli) — the `nopass` CLI binary
- `apps/web` — marketing site *(planned)*
- `apps/desktop` — desktop app, Tauri over nopass-core *(planned)*
- `apps/mobile` — mobile app *(planned)*

## Install

```sh
cargo install --path crates/nopass-cli
# or just build it
cargo build --release && ./target/release/nopass --help
```

Optional: `devbox shell` provides rust + git (+ gnupg for the gpg backend).

## Quick start

```sh
nopass init                       # generates your keypair + creates ~/.nopass
nopass git init                   # optional: track history in git
nopass generate web/github 25     # generate + store a password
nopass insert mail/proton         # type a password in manually
nopass show web/github            # print it
nopass show -c web/github         # copy first line to clipboard, auto-clear
nopass edit mail/proton           # open in $EDITOR
nopass                            # list the whole tree
nopass find git                   # search entry names
nopass grep alice                 # search decrypted contents
nopass mv web/github work/github  # move + re-encrypt
nopass rm -f mail/proton          # delete
```

Your secret key lives at `~/.config/nopass/identity.txt` (created by
`nopass init` or `nopass keygen`). **Back it up** — without it your store
cannot be decrypted. Sharing a folder with someone? Add their public key
(`age1...`, printed by their `nopass keygen`) to that folder's `.nopass-id`
and run `nopass init -p <folder> <key1> <key2>`.

## Commands

```
nopass keygen [--force]                  generate this machine's keypair
nopass init [-p subfolder] [recipients]  initialize store (auto-keygen if needed)
nopass [ls] [subfolder]                  list entries
nopass show [-c[line]] name              decrypt and print (or copy to clipboard)
nopass find terms...                     list entries matching terms
nopass grep pattern                      search decrypted contents
nopass insert [-e|-m] [-f] name          add an entry
nopass edit name                         edit with $EDITOR
nopass generate [-n] [-c] [-i|-f] name [length]
nopass rm [-r] [-f] name                 remove entry or directory
nopass mv [-f] old new                   move + re-encrypt
nopass cp [-f] old new                   copy + re-encrypt
nopass git <args>...                     run git in the store
```

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `NOPASS_DIR` | `~/.nopass` | store location |
| `NOPASS_IDENTITY` | `~/.config/nopass/identity.txt` | secret key file |
| `NOPASS_BACKEND` | `native` | `native`, `gpg`, or `plain` (tests only) |
| `NOPASS_KEY` | — | override recipients for all encryption |
| `NOPASS_GENERATED_LENGTH` | `25` | default generated password length |
| `NOPASS_CHARACTER_SET` | alnum + punct | generation charset |
| `NOPASS_CLIP_TIME` | `45` | seconds before clipboard clears |
| `NOPASS_GPG_OPTS` | — | extra flags for the gpg backend |

## Development

```sh
devbox shell
cargo test --workspace
cargo clippy --workspace --all-targets
```

The test suite is fully hermetic: native-backend tests use throwaway
identities in temp dirs, and a `plain` backend covers store mechanics
without any cryptography.

## License

MIT.
