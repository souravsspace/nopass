# nopass, built from this checkout.
#
# Used by the flake at the repository root: `nix build`, `nix run`, or
# `nix profile install github:souravsspace/nopass`. Dependencies come from
# Cargo.lock, so there is no vendor hash to keep up to date.
#
# For the version that builds a published release instead — the shape
# nixpkgs wants — see ./nopass-release.nix.
{
  lib,
  rustPlatform,
  stdenv,
  git,
  pkg-config,
  udev,
  # The FIDO2 security-key slot pulls in a C HID stack. Off by default,
  # exactly as in Cargo.toml.
  withSecurityKey ? false,
}:

rustPlatform.buildRustPackage {
  pname = "nopass";
  version = (lib.importTOML ../../Cargo.toml).workspace.package.version;

  src = lib.cleanSource ../..;
  cargoLock.lockFile = ../../Cargo.lock;

  cargoBuildFlags = [ "--package" "nopass-cli" ];
  buildFeatures = lib.optional withSecurityKey "security-key";

  nativeBuildInputs = lib.optionals withSecurityKey [ pkg-config ];
  buildInputs = lib.optionals (withSecurityKey && stdenv.hostPlatform.isLinux) [ udev ];

  # The suite is hermetic — no network, no real store, temp dirs throughout —
  # but the history and sync tests do drive a real git, and every test wants
  # a HOME of its own to keep away from the one running the build.
  doCheck = true;
  nativeCheckInputs = [ git ];
  preCheck = ''
    export HOME=$(mktemp -d)
  '';

  meta = {
    description = "Fast, self-contained password manager";
    longDescription = ''
      nopass keeps each password in its own age-encrypted file under one
      directory. Every command that reads or changes the store asks for your
      master passphrase; reads can reuse a cached one when you opt in.
    '';
    homepage = "https://github.com/souravsspace/nopass";
    changelog = "https://github.com/souravsspace/nopass/releases";
    license = lib.licenses.mit;
    mainProgram = "nopass";
    platforms = lib.platforms.unix;
  };
}
