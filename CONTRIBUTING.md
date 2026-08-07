# Contributing to nopass

Thanks for looking. nopass is a small, self-contained password manager, and
it stays useful by staying small. This page is what a change has to look
like to get merged.

## What belongs here

**In scope:** correctness and security fixes, better error messages, sharper
docs, portability across Linux/macOS shells, and features that a password
manager cannot reasonably do without.

**Out of scope, unless you have talked it through in an issue first:** new
storage formats, cloud sync services, GUIs, plugin systems, and configuration
knobs for behaviour that has one sensible answer. A flag nobody sets is a
flag everybody has to read about.

If you are unsure, open an issue describing the problem — not the patch —
before writing code.

## Setting up

```sh
devbox shell          # rust + git (+ gnupg, for the optional gpg backend)
cargo build
cargo test --workspace
```

No devbox? Any stable Rust toolchain (1.74+) works; `rustup component add
clippy rustfmt`.

Optional features:

```sh
cargo test --workspace --features security-key   # FIDO2 slots; needs libudev on Linux
```

## The repo

```
crates/nopass-core/     the engine — no CLI, no prompts, no printing
  config.rs             the ~/.config/nopass/config pointer file
  crypto.rs             encryption backends (native age, gpg, plain-for-tests)
  error.rs              one error enum, user-facing messages included
  generate.rs           password generation
  git.rs                auto-commit and auto-sync of the store
  lock.rs               the locked identity file and its key slots
  paths.rs              path safety, recipient files
  store.rs              the store: entries, recipients, moves, re-encryption
crates/nopass-cli/      the `nopass` binary
  main.rs               argument parsing, every command, prompts, clipboard
  auth.rs               unlocking: security key, passphrase, cache policy
  agent.rs              the in-memory passphrase cache and its unix socket
  fido2.rs              security keys, and the software mock used by tests
  setup.rs              first-run key creation
packaging/homebrew/     the tap formula
```

Rule of thumb: anything that prompts, prints, or reads `argv` lives in the
CLI; anything that could be reused by another front end lives in core.

## House rules for changes

These exist because the codebase is small enough to read in an afternoon,
and should stay that way.

1. **Smallest change that solves the problem.** No abstraction for a single
   caller, no configurability that was not asked for, no error handling for
   situations that cannot happen.
2. **Surgical diffs.** Do not reformat, rename, or "improve" code your change
   did not need to touch. Match the style around you even if you would have
   written it differently. Noticed unrelated dead code? Mention it in the PR
   rather than deleting it.
3. **Every changed line should trace back to the stated problem.** If you
   cannot explain why a line is in the diff, take it out.
4. **Comments explain *why*.** The code already says what it does. Existing
   comments are written in prose, in full sentences — keep that voice.
5. **No AI attribution anywhere** — not in commit messages, not in PR
   descriptions, not in code comments. The commit author is the person who
   sent the change.

## Tests come first

Write the failing test, then the code that passes it. A bug fix without a
test that reproduces the bug will be asked for one.

```sh
cargo test --workspace          # 150 tests, hermetic: no network, no real store
cargo clippy --workspace --all-targets
cargo fmt --all
```

All three must be clean before you open a PR.

Tests never touch your real store, keys, or the network. Three harnesses in
`crates/nopass-cli/tests/cli.rs` cover the CLI end to end:

- **`TestStore`** — store mechanics against `NOPASS_BACKEND=plain`, a
  deliberately fake backend that only prefixes a header. Fast, and about
  paths and files rather than crypto.
- **`NativeStore`** — real age encryption with a throwaway identity in a temp
  dir. Use it for anything about keys, slots, or security keys
  (`NOPASS_FIDO2_MOCK` swaps in a software authenticator).
- **`FreshMachine`** — a whole imaginary machine: its own `HOME`, its own
  config, no key at all to begin with. Use it for first-run setup,
  passphrase prompts, and the passphrase cache. Prompts are driven by piping
  answers to stdin, in the order a person would type them.

Unit tests live next to the code they test, at the bottom of the file.

Name tests after the behaviour they pin down, in a sentence:
`writing_a_password_asks_for_the_passphrase_too`, not `test_insert_2`.

## Working on the security-sensitive parts

Extra care is expected in `lock.rs`, `crypto.rs`, `auth.rs`, `agent.rs` and
`setup.rs`. When you change them, say in the PR which of these you checked:

- **The secret key never reaches the disk in the clear.** A locked identity
  is only ever written through `write_secret_file`, already encrypted.
- **The unlocked key never leaves memory**, and never enters a command line,
  an environment variable, a log line, or an error message.
- **Prompts cannot be skipped by piping.** There is no TTY check anywhere on
  purpose: end-of-input is an error, not a silent default.
- **Entry names are attacker-controlled.** Everything that resolves a name
  goes through `paths::check_sneaky_path`.
- **The passphrase cache is opt-in, memory-only, and read-only** — writes
  always ask the user. Do not add a code path where a cached secret
  authorises a change to the store.
- **Nothing new gets written to a world-writable directory.**

Please state the threat model of a change in the PR: what an attacker can do
before, and what they can do after.

### Reporting a vulnerability

Do not open a public issue. Email the maintainer (see the GitHub profile for
`souravsspace`) with the version, the steps, and what an attacker gains. You
will get an acknowledgement, and credit in the release notes unless you would
rather not have it.

## Commits and pull requests

- **One commit per file.** Each commit message describes that file's change.
  It reads oddly at first and pays for itself when you bisect.
- Imperative mood, no trailing period, and the *why* in the body when the
  subject cannot carry it:

  ```
  Ask for the passphrase before writing an entry

  Encrypting only needs the public key, so adding or deleting an entry used
  to need no authentication at all — a borrowed terminal was enough to empty
  someone's store.
  ```

- Rebase on `main` rather than merging it in; keep the history linear.
- The PR description should say what problem you hit, how you fixed it, and
  how you know it works. Link the issue if there is one.
- Expect review comments about scope more than about style. Shrinking a patch
  is a compliment.

## Releasing

Maintainers only, and the whole procedure lives in
[RELEASING.md](RELEASING.md): bump the workspace version, tag, create the
GitHub release (`nopass update` reads it), then update the Homebrew tap.

## License

nopass is MIT-licensed. By contributing you agree that your contribution is
released under the same license, and that you have the right to release it.
