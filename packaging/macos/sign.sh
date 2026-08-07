#!/usr/bin/env bash
#
# Build a signed macOS nopass that can use the Touch ID slot.
#
# The Secure Enclave key nopass enrolls for Touch ID lives in the
# data-protection keychain, and macOS only opens that for a binary whose
# signature carries an application identifier plus a matching keychain access
# group — both authorized by an embedded provisioning profile. A profile can
# only be embedded in a bundle, so the CLI ships inside nopass.app and the
# installed `nopass` is a symlink to the executable within it.
#
# Everything else about nopass works from an ordinary `cargo install`; this is
# only what Touch ID costs.
#
# Usage:
#   TEAM_ID=ABCDE12345 \
#   BUNDLE_ID=com.example.nopass \
#   SIGN_IDENTITY="Developer ID Application: Your Name (ABCDE12345)" \
#   PROFILE=~/Downloads/nopass.provisionprofile \
#   packaging/macos/sign.sh
#
# Optional:
#   NOTARY_PROFILE=nopass-notary   # keychain profile for `xcrun notarytool`
#   TARGET=aarch64-apple-darwin    # cross-compile target
#
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
here=$repo_root/packaging/macos
dist=$repo_root/dist
app=$dist/nopass.app

die() { printf 'error: %s\n' "$*" >&2; exit 1; }
step() { printf '\n==> %s\n' "$*"; }

: "${TEAM_ID:?set TEAM_ID to your 10-character Apple team identifier}"
: "${BUNDLE_ID:?set BUNDLE_ID, e.g. com.example.nopass — it must match your App ID}"
: "${SIGN_IDENTITY:?set SIGN_IDENTITY to a codesigning identity (security find-identity -v -p codesigning)}"
: "${PROFILE:?set PROFILE to the .provisionprofile downloaded for BUNDLE_ID}"
[ -f "$PROFILE" ] || die "no provisioning profile at $PROFILE"

step "Building release binary"
build_args=(--release --locked -p nopass-cli)
[ -n "${TARGET:-}" ] && build_args+=(--target "$TARGET")
(cd "$repo_root" && cargo build "${build_args[@]}")
binary=$repo_root/target/${TARGET:+$TARGET/}release/nopass
[ -x "$binary" ] || die "no binary at $binary"

step "Assembling $app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp "$binary" "$app/Contents/MacOS/nopass"
cp "$PROFILE" "$app/Contents/embedded.provisionprofile"

version=$(cd "$repo_root" && cargo metadata --no-deps --format-version 1 |
  sed -n 's/.*"name":"nopass-cli","version":"\([^"]*\)".*/\1/p' | head -1)
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>nopass</string>
  <key>CFBundleIdentifier</key><string>${BUNDLE_ID}</string>
  <key>CFBundleName</key><string>nopass</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version:-0.0.0}</string>
  <key>CFBundleVersion</key><string>${version:-0.0.0}</string>
  <key>LSMinimumSystemVersion</key><string>10.15</string>
</dict>
</plist>
PLIST

step "Signing with $SIGN_IDENTITY"
entitlements=$dist/nopass.entitlements
sed -e "s/TEAM_ID/$TEAM_ID/g" -e "s/BUNDLE_ID/$BUNDLE_ID/g" \
  "$here/nopass.entitlements" > "$entitlements"
codesign --force --timestamp --options runtime \
  --entitlements "$entitlements" \
  --sign "$SIGN_IDENTITY" \
  "$app/Contents/MacOS/nopass"
codesign --force --timestamp --options runtime \
  --entitlements "$entitlements" \
  --sign "$SIGN_IDENTITY" \
  "$app"

step "Verifying the signature"
codesign --verify --strict --deep --verbose=2 "$app"
codesign -d --entitlements - "$app/Contents/MacOS/nopass" 2>/dev/null | sed 's/^/    /'

# The real test. Entitlements the profile does not authorize are not a signing
# error — the kernel kills the process the moment it launches, so a binary that
# merely signs cleanly can still be dead on arrival.
step "Checking the signed binary actually runs"
if ! "$app/Contents/MacOS/nopass" --version >/dev/null 2>&1; then
  status=$?
  if [ "$status" -eq 137 ]; then
    die "the signed binary was killed at launch (SIGKILL). The provisioning
       profile does not authorize these entitlements: check that its App ID is
       exactly $BUNDLE_ID, that it includes the keychain access group
       $TEAM_ID.$BUNDLE_ID, and that this Mac is one of its devices."
  fi
  die "the signed binary exited with status $status"
fi

if [ -n "${NOTARY_PROFILE:-}" ]; then
  step "Notarizing"
  zip=$dist/nopass-macos.zip
  rm -f "$zip"
  ditto -c -k --keepParent "$app" "$zip"
  xcrun notarytool submit "$zip" --keychain-profile "$NOTARY_PROFILE" --wait
  # A bare CLI cannot be stapled; Gatekeeper checks notarization online. The
  # zip is what you attach to the GitHub release.
  step "Notarized archive: $zip"
else
  step "Skipping notarization (set NOTARY_PROFILE to enable)"
fi

cat <<INSTALL

Done: $app

Install it so \`nopass\` on PATH is the signed build:

    sudo rm -rf /usr/local/lib/nopass.app
    sudo cp -R "$app" /usr/local/lib/nopass.app
    sudo ln -sf /usr/local/lib/nopass.app/Contents/MacOS/nopass /usr/local/bin/nopass

Then enroll Touch ID:

    nopass passkey enroll

INSTALL
