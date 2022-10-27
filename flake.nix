{
  description = "Flux Screensavers";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane = {
      url = "github:ipetkov/crane";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = { self, nixpkgs, flake-utils, crane, rust-overlay }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          crossSystem.config = "x86_64-w64-mingw32";
          overlays = [ (import rust-overlay) ];
        };

        rustToolchain = pkgs.pkgsBuildHost.rust-bin.stable.latest.default.override {
          targets = [ "x86_64-pc-windows-gnu" ];
        };

        craneLib = (crane.mkLib pkgs).overrideScope' (final: prev: {
          rustc = rustToolchain;
          cargo = rustToolchain;
          rustfmt = rustToolchain;
        });
      in rec {
        devShells = {
          default = pkgs.pkgsBuildHost.mkShell {
            inputsFrom = [ packages.flux-screensaver-windows ];
            packages = with pkgs.pkgsBuildHost; [ rustToolchain nixfmt ];
          };
        };

        packages = {
          default = craneLib.buildPackage rec {
            src = ./windows;
            release = true;

            buildInputs = [
              pkgs.windows.pthreads
              pkgs.windows.mingw_w64_pthreads
            ];

            CARGO_BUILD_TARGET = "x86_64-pc-windows-gnu";
            CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = "${pkgs.stdenv.cc.targetPrefix}cc";

            # Change the extension to .scr (Windows screensaver)
            postInstall = ''
              if [[ $out != *"deps"* ]]; then
                mv $out/bin/Flux.exe "$out/bin/Flux.scr"
              fi
            '';
          };
        };
      });
}
