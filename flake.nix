{
  # A flake has to live at the repository root to be usable as
  # `github:souravsspace/nopass`; the package itself is in
  # packaging/nix/nopass.nix.
  description = "nopass: a fast, self-contained password manager";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSystem = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forEachSystem (pkgs: rec {
        nopass = pkgs.callPackage ./packaging/nix/nopass.nix { };
        # The FIDO2 slot needs a C HID stack, so it stays a separate build.
        nopass-with-security-key = nopass.override { withSecurityKey = true; };
        default = nopass;
      });

      apps = forEachSystem (pkgs: rec {
        nopass = {
          type = "app";
          program = "${self.packages.${pkgs.system}.nopass}/bin/nopass";
        };
        default = nopass;
      });

      devShells = forEachSystem (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            clippy
            rustfmt
            git
            gnupg
          ];
        };
      });

      formatter = forEachSystem (pkgs: pkgs.nixfmt-rfc-style);
    };
}
