{
  description = "pinentry that answers the OpenPGP card PIN from the macOS Keychain or Windows Credential Manager";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-26.05";

    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
  };

  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake {inherit inputs;} ({lib, ...}: let
      mkPinentry = pkgs: pkgs.callPackage ./package.nix {};
    in {
      # macOS builds the native pinentry; x86_64-linux cross-builds the Windows one (its MinGW toolchain is
      # in the binary cache) and runs the tests natively.
      systems = ["aarch64-darwin" "x86_64-linux"];

      perSystem = {
        pkgs,
        system,
        ...
      }: {
        packages =
          lib.optionalAttrs pkgs.stdenv.hostPlatform.isDarwin {
            default = mkPinentry pkgs;
          }
          // lib.optionalAttrs (system == "x86_64-linux") {
            windows = mkPinentry pkgs.pkgsCross.mingwW64;
          };

        checks.tests = mkPinentry pkgs;

        devShells.default = pkgs.mkShell {
          packages = [pkgs.cargo pkgs.rustc pkgs.clippy pkgs.rustfmt];
        };

        formatter = pkgs.alejandra;
      };

      # Platform checks read `prev`: the attribute names must not depend on `final`.
      flake.overlays.default = final: prev:
        lib.optionalAttrs prev.stdenv.hostPlatform.isDarwin {
          credential-store-pinentry = mkPinentry final;
        }
        // lib.optionalAttrs prev.stdenv.hostPlatform.isx86_64 {
          credential-store-pinentry-windows = mkPinentry final.pkgsCross.mingwW64;
        };
    });
}
