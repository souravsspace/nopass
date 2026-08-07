# nopass, built from a published release rather than a checkout.
#
# This is the shape nixpkgs expects, so it is also the file to send upstream
# (pkgs/by-name/no/nopass/package.nix). Build it on its own with:
#
#   nix-build -E 'with import <nixpkgs> {}; callPackage ./nopass-release.nix {}'
#
# Two hashes to bump per release. Neither is the tarball checksum the
# Homebrew and AUR packages use — fetchFromGitHub hashes the unpacked tree —
# so get both the same way: set them to lib.fakeHash, build, and copy the
# value the mismatch error prints.
#
#   src.hash    the unpacked source tree
#   cargoHash   the vendored crate dependencies; changes with Cargo.lock
{
  lib,
  fetchFromGitHub,
  rustPlatform,
  stdenv,
  git,
  pkg-config,
  udev,
  withSecurityKey ? false,
}:

rustPlatform.buildRustPackage rec {
  pname = "nopass";
  version = "0.2.1";

  src = fetchFromGitHub {
    owner = "souravsspace";
    repo = "nopass";
    tag = "v${version}";
    hash = "sha256-b2H7xPI5Nt2Zwe4YGpigvxh/OFfPw5Gq54dRKuB3lU8=";
  };

  cargoHash = "sha256-zmDWXbcjraMfizO4vYTVeH2gKWbb75FK7vACNV92qzo=";

  cargoBuildFlags = [ "--package" "nopass-cli" ];
  buildFeatures = lib.optional withSecurityKey "security-key";

  nativeBuildInputs = lib.optionals withSecurityKey [ pkg-config ];
  buildInputs = lib.optionals (withSecurityKey && stdenv.hostPlatform.isLinux) [ udev ];

  # The history and sync tests drive a real git, and every test wants a HOME
  # of its own to keep away from the one running the build.
  nativeCheckInputs = [ git ];
  preCheck = ''
    export HOME=$(mktemp -d)
  '';

  meta = {
    description = "Fast, self-contained password manager";
    homepage = "https://github.com/souravsspace/nopass";
    changelog = "https://github.com/souravsspace/nopass/releases/tag/v${version}";
    license = lib.licenses.mit;
    mainProgram = "nopass";
    platforms = lib.platforms.unix;
  };
}
