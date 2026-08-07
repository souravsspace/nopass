# nopass

Fast, self-contained password manager. Each entry lives in its own
age-encrypted file under one directory, and every command that reads or
changes the store asks for your master passphrase.

```sh
npm install -g nopass-cli
nopass init
nopass insert github.com
nopass github.com
```

Full documentation: <https://github.com/souravsspace/nopass>

## What this package does

It downloads the prebuilt binary for your platform from the matching GitHub
release, checks it against the release's published sha256, and puts it on
your PATH. macOS and Linux, x64 and arm64.

There is no Rust toolchain involved. If you would rather build from source:

```sh
cargo install nopass-cli
```

Windows is not supported by any install method yet — nopass uses unix
sockets, `stty` and `/dev/shm`.
