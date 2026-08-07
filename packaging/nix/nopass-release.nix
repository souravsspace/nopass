# nopass, built from a published release rather than a checkout.
#
# This is the shape nixpkgs expects, so it is also the file to send upstream
# (pkgs/by-name/no/nopass/package.nix). Build it on its own with:
#
#   nix-build -E 'with import <nixpkgs> {}; callPackage ./nopass-release.nix {}'
#
# Two hashes to bump per release:
#   src.hash    nix hash convert --hash-algo sha256 --to sri \
#                 "$(curl -fsSL $url | sha256sum | cut -d' ' -f1)"
#   cargoHash   set it to lib.fakeHash, build, and copy the hash the error prints
{
  lib,
  fetchFromGitHub,
  rustPlatform,
  stdenv,
  pkg-config,
  udev,
  withSecurityKey ? false,
}:

rustPlatform.buildRustPackage rec {
  pname = "nopass";
  version = "0.2.0";

  src = fetchFromGitHub {
    owner = "souravsspace";
    repo = "nopass";
    tag = "v${version}";
    hash = "sha256-Uc1k+B4Rh/Xvd2v1zHqqGEHWO9VV+9A7XGCzBNT3d5A=";
  };

  # Replace with lib.fakeHash and build once to learn the real value after
  # any change to Cargo.lock.
  cargoHash = lib.fakeHash;

  cargoBuildFlags = [ "--package" "nopass-cli" ];
  buildFeatures = lib.optional withSecurityKey "security-key";

  nativeBuildInputs = lib.optionals withSecurityKey [ pkg-config ];
  buildInputs = lib.optionals (withSecurityKey && stdenv.hostPlatform.isLinux) [ udev ];

  meta = {
    description = "Fast, self-contained password manager";
    homepage = "https://github.com/souravsspace/nopass";
    changelog = "https://github.com/souravsspace/nopass/releases/tag/v${version}";
    license = lib.licenses.mit;
    mainProgram = "nopass";
    platforms = lib.platforms.unix;
  };
}
