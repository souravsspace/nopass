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

Or via Homebrew (once the tap is published):

```sh
brew tap souravsspace/tap
brew install nopass
```

Or for development, [devbox](https://www.jetify.com/devbox) provides the
whole toolchain:

```sh
devbox shell
cargo build --release   # binary at target/release/nopass
```

### Updating

```sh
nopass update --check    # see if a newer release exists
nopass update            # install it (auto-detects brew vs cargo install)
```

Homebrew users can equally run `brew upgrade nopass`. Maintainer release
flow lives in [RELEASING.md](RELEASING.md).

## Getting started, A to Z

### 1. Create your key and store

```sh
nopass init
```

The first run asks you two things:

```
Where should the private key live?
  [1] /home/you/.config/nopass/identity.txt   (default)
  [2] a directory you choose (nopass makes a nopass/ folder in it)
Choice [1]:

Choose a master passphrase. nopass asks for it every time it reads
a password, and it is the only thing protecting the key file.

Master passphrase:
Retype master passphrase:
```

Then it prints where the key went, your public key, and creates the store at
`~/.nopass`. Everything you store is encrypted to that one keypair, and the
secret half is written **already locked** with your passphrase — it never
touches the disk in the clear.

> **Back up the private key file somewhere safe.** Without it — or without
> the passphrase — nobody can read your passwords, including you.

Pick option 2 and name any directory — `~/Desktop`, a synced folder, an
encrypted volume. nopass creates its own `nopass/` folder inside it (so you
get `~/Desktop/nopass/identity.txt`, never a loose `identity.txt`) and
remembers the location in `~/.config/nopass/config`, so every later command
finds it with no environment variables to set.

You can also say it up front:

```sh
nopass init --identity ~/Vaults          # a directory -> ~/Vaults/nopass/identity.txt
nopass init --identity ~/Vaults/work.txt # a filename is taken literally
```

Reading a password asks for the passphrase every time, like `pass` does.
Nothing is cached between commands; within one command it asks once, however
many entries that command has to decrypt.

You can also create the key explicitly first, or skip the passphrase:

```sh
nopass keygen                     # same questions, without creating the store
nopass keygen --force             # replace existing key (old entries become unreadable!)
nopass init --no-passphrase       # unattended: unprotected key, never prompts
```

`--no-passphrase` leaves the key readable by anything that can read the file.
Lock it later with `nopass passkey enroll` (see below).

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
nopass keygen [--force] [--identity path] [--no-passphrase]
                                         generate this machine's keypair
nopass init [-p subfolder] [--identity path] [--no-passphrase] [recipients]
                                         initialize store (auto-keygen if needed)
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
nopass update [--check]                  update nopass itself
nopass help                              full command tour + your file paths
nopass passkey enroll [--security-key] [--pin]
                                         lock the identity behind auth
nopass passkey add-key [--label name] [--pin]
                                         enroll another security key
nopass passkey remove-key name           drop a security key slot
nopass passkey disable                   remove the lock (requires auth)
nopass passkey status                    show lock state and slots
nopass lock                              forget the cached passphrase now
nopass agent status                      what is cached, and for how long
nopass agent stop                        forget it and stop the agent
```

Aliases: `ls`=`list`, `rm`=`remove`/`delete`, `mv`=`rename`, `cp`=`copy`.

## Locking your identity (passkey)

A key made by `nopass init` is already locked with the master passphrase you
chose. `passkey` is how you change what opens it — add a security key, change
the passphrase, or lock a key that was created with `--no-passphrase`:

```sh
nopass passkey enroll                   # passphrase only
nopass passkey enroll --security-key    # …plus a FIDO2 key, e.g. a YubiKey
```

Running `enroll` on an already-locked identity asks to unlock it first, then
re-locks it with the new passphrase — that is how you change it.

### Forgot your master passphrase?

Then the key is unrecoverable, and so is every password encrypted to it.
There is no backdoor and no reset: the key file is sealed with your
passphrase (scrypt + ChaCha20-Poly1305) and nopass holds nothing else that
can open it. Changing a passphrase requires knowing the current one.

Set up a way back **before** you need it. Any one of these is enough:

- **Enroll a second factor.** `nopass passkey enroll --security-key`. Slots
  are independent — a security key you still have unlocks the identity, and
  `nopass passkey enroll` then sets a new passphrase.
- **Keep an offline copy of the unlocked key.** `nopass passkey disable`
  writes the plaintext key back to the identity file; copy that file to a USB
  stick or paper you keep somewhere safe, then re-lock with
  `nopass passkey enroll`. Anyone holding that copy can read your store, so
  treat it like a house key.
- **Write the passphrase down** and keep it somewhere physical.

Backing up the *locked* identity file protects you from a dead disk, not from
a forgotten passphrase — the backup is sealed with the same passphrase.

Every command that decrypts an entry (`show`, `grep`, `edit`, `generate -i`,
`mv`, `cp`) prompts to unlock first; reading the identity file directly
reveals nothing. Writing (`insert`, `generate`) needs only the public key, so
it never prompts.

Locking uses independent **slots**, like disk encryption: unlocking any one
slot recovers the identity.

- **Passphrase slot** — always created, works on every platform. The
  passphrase is run through scrypt; the identity is sealed with age.
- **Security key slots** *(FIDO2, optional)* — any number of hardware keys,
  via the CTAP2 `hmac-secret` extension. See below.
Unlocking tries any enrolled security key, then the passphrase, falling
through whenever the key is missing or refuses. `NOPASS_UNLOCK=passphrase`
skips straight to typing.

```sh
nopass passkey status        # see whether it's locked and which slots exist
nopass passkey disable       # unlock (prompts), then store plaintext again
```

Re-running `enroll` on an already-locked identity unlocks it first, so you can
change the passphrase or add slots later.

### Biometric unlock

nopass has no biometric slot on any platform, by design.

Touch ID was implemented and removed: macOS only hands out biometry-gated
Secure Enclave keys to binaries signed with keychain entitlements authorized
by an embedded provisioning profile, so no build from source — `cargo
install`, or Homebrew compiling the formula — could ever use it. Windows Hello
may be reachable through the WebAuthn `hmac-secret` extension, but that is
unproven. Linux fingerprint readers (fprintd) only prove someone touched the
sensor; they release no key material, so a slot built on one would be
bypassed by reading the identity file — worse than not having it.

A FIDO2 security key gives real hardware-backed unlock on all three platforms
today, from any build.

### Security keys (passkeys)

A FIDO2 security key becomes a slot that unlocks the store with a touch and no
typing:

```sh
cargo install --path crates/nopass-cli --features security-key

nopass passkey enroll --security-key           # lock, enrolling a key
nopass passkey add-key --label backup          # add a second key later
nopass passkey remove-key backup               # and drop it again
```

How it works: enrollment creates a credential on the key with the CTAP2
`hmac-secret` extension, then asks it for `HMAC-SHA256(credRandom, salt)` over
a random 32-byte salt (so you may be prompted to touch twice — CTAP only
returns the HMAC from an assertion, never from registration). That output is a
uniform 256-bit key and seals the slot. The identity file records only the
credential id and the salt, both public; `credRandom` never leaves the key, so
the slot cannot be opened without the physical device.

- Add `--pin` to require the key's PIN as well as a touch.
- Enroll **more than one key** if you rely on this — a key that is lost, wiped
  or reset takes its slot with it.
- The passphrase slot is always kept as a fallback, so a forgotten key at the
  office never locks you out of your own store.
- The `security-key` feature is off by default because it pulls in a C HID
  stack (on Linux, `libudev` headers are needed to build).

Slots recorded by a newer nopass are ignored rather than fatal to an older
one, so a store shared across machines keeps working while you roll out.

## Authentication and the passphrase cache

Every command that touches the store proves the store is yours first:

- **Reading** — `show`, `grep`, `edit` — has to unlock the key to decrypt.
- **Changing** — `insert`, `generate`, `edit`, `rm`, `mv`, `cp`, `init` —
  asks too, even though encrypting only needs the public half. Deleting an
  entry needs no key at all, so this prompt is the only thing between a
  borrowed terminal and an emptied store.

By default nothing is remembered between commands. If that is too much
typing, let reads reuse a passphrase for a while:

```sh
echo "cache-ttl = 300" >> ~/.config/nopass/config   # or NOPASS_CACHE_TTL=300
```

The first unlock then starts a small agent that keeps the unlocked key **in
memory only** — never on disk — for that many seconds, listening on a socket
in a directory only you can reach. What it does and does not cover:

- Reads are served from the cache; **every change still asks**, and typing it
  again does not extend the entry.
- The lifetime is absolute, not idle-based, and counted on the wall clock, so
  a laptop that sleeps through it wakes up with nothing cached.
- Relocking the identity (`passkey enroll`, `add-key`, `remove-key`) leaves
  the old entry unusable rather than stale.
- `nopass lock` forgets it now; `nopass agent status` says what is held.
- The cache needs a private runtime directory (`$XDG_RUNTIME_DIR`, or the
  per-session `$TMPDIR` on macOS). Without one, nopass just keeps asking.

## Configuration

All optional, via environment variables:

| Variable | Default | Purpose |
|---|---|---|
| `NOPASS_DIR` | `~/.nopass` | store location |
| `NOPASS_IDENTITY` | config file, else `~/.config/nopass/identity.txt` | secret key file |
| `NOPASS_CONFIG` | `~/.config/nopass/config` | file recording the key location |
| `NOPASS_BACKEND` | `native` | `native`, `gpg`, or `plain` (tests only) |
| `NOPASS_AUTOSYNC` | `1` | `0` disables auto pull/push to the remote |
| `NOPASS_KEY` | — | override recipients for all encryption |
| `NOPASS_GENERATED_LENGTH` | `25` | default generated password length |
| `NOPASS_CHARACTER_SET` | alnum + punct | generation charset |
| `NOPASS_CHARACTER_SET_NO_SYMBOLS` | alnum | charset for `generate -n` |
| `NOPASS_CLIP_TIME` | `45` | seconds before clipboard clears |
| `NOPASS_GPG_OPTS` | — | extra flags for the gpg backend |
| `NOPASS_UNLOCK` | — | `passphrase` skips enrolled security keys |
| `NOPASS_CACHE_TTL` | `0` | seconds a *read* may reuse an unlocked key |
| `NOPASS_AGENT_SOCK` | `$XDG_RUNTIME_DIR/nopass/agent.sock` | where the agent listens |
| `NOPASS_FIDO2_MOCK` | — | software test authenticator state file (tests only) |

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
- The secret identity is encrypted at rest with your master passphrase (mode
  0600) unless you created it with `--no-passphrase`. Run
  `nopass passkey enroll` to add a FIDO2 security key, or to lock a key that
  was created unprotected.
- The unlocked key is held in memory for the life of a single command, and
  never written to disk. With `cache-ttl` set it is also held by the agent —
  still in memory, still never on disk — for that many seconds, and only
  reads may use it.
- The prompt on `insert`, `rm` and friends is a check by the program, not a
  cryptographic one: anything running as you can write entries with the
  public key alone, or delete files from the store directly. It defends a
  borrowed terminal, not a compromised account.
- `NOPASS_FIDO2_MOCK` swaps the real authenticator for a file-backed software
  one. It exists for the test suite — like `NOPASS_BACKEND=plain` — and warns
  loudly; slots enrolled that way are only as safe as that file.
- The clipboard is cleared after `NOPASS_CLIP_TIME` seconds, but other apps
  may read the clipboard during that window.
- `nopass edit` writes plaintext to a temp dir (`/dev/shm` ramdisk when
  available) for the duration of the edit. The directory is created 0700 —
  never adopted, so a planted one is an error rather than somewhere to leave
  a password — the file is 0600, and both are removed however the command
  ends, including when the editor fails.
- Entry names are confined to the store: `..` and absolute paths are refused
  everywhere, so no command can be aimed outside `NOPASS_DIR`.
- With `NOPASS_BACKEND=gpg` the write prompt is gpg's, not nopass's: gpg-agent
  decides how often you are asked. With `NOPASS_BACKEND=plain` there is no
  key at all, so nothing is encrypted and nothing is authenticated — it warns
  on every command and exists only for the test suite.
- `keygen --force` orphans anything encrypted only to the old key. Add the
  new key as a recipient and re-init instead if you want a rotation:
  `nopass init age1newkey...` re-encrypts everything.

## Development

```sh
devbox shell                          # rust + git (+ gnupg for gpg backend)
cargo test --workspace                # 162 tests, fully hermetic
cargo clippy --workspace --all-targets
cargo fmt --all
```

Tests never touch your real store, keys, or network: store mechanics run
against a plaintext test backend, native-crypto tests use throwaway
identities in temp dirs, and sync tests push to local bare repos.

Patches welcome — [CONTRIBUTING.md](CONTRIBUTING.md) covers the layout, the
test harnesses, and the house rules.

## License

MIT.
