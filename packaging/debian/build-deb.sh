#!/bin/sh
# Build a .deb from the current checkout.
#
# cargo-deb reads Cargo.toml and fills in the rest, so there is no debian/
# directory to keep in step with the crate. The result lands in
# target/debian/nopass_<version>_<arch>.deb.
#
#   cargo install cargo-deb
#   packaging/debian/build-deb.sh
#
# Install it with `sudo apt install ./nopass_0.2.0_arm64.deb`.
set -eu

cd "$(dirname "$0")/../.."

command -v cargo-deb >/dev/null || {
	echo "cargo-deb is not installed: cargo install cargo-deb" >&2
	exit 1
}

cargo deb \
	--package nopass-cli \
	--maintainer "Sourav <souravsspace@gmail.com>" \
	-- --locked

echo
echo "Built:"
ls -1 target/debian/*.deb
