{
  description = "Flux Screensavers";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-22.05";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nmattia/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, fenix, naersk }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        rustToolchain = with fenix.packages.${system};
          combine ([
            latest.rustc
            latest.cargo
            latest.clippy
            targets.x86_64-pc-windows-gnu.latest.rust-std
          ]);

        naersk-lib = naersk.lib.${system}.override {
          rustc = rustToolchain;
          cargo = rustToolchain;
        };
      in rec {
        devShells = {
          default = pkgs.mkShell {
            inputsFrom = [ packages.default ];
            packages = with pkgs; [ rustToolchain nixfmt ripgrep ];
          };

          cross-windows = pkgs.mkShell {
            inputsFrom = [ packages.flux-screensaver-windows ];
            packages = with pkgs; [ rustToolchain nixfmt ripgrep ];
          };
        };

        packages = {
          default = let
            SDL2_static = pkgs.SDL2.overrideAttrs (old: rec {
              version = "2.0.22";
              name = "SDL2-static-${version}";
              src = builtins.fetchurl {
                url =
                  "https://www.libsdl.org/release/${old.pname}-${version}.tar.gz";
                sha256 =
                  "sha256:0bkzd5h7kn4xmd93hpbla4n2f82nb35s0xcs4p3kybl84wqvyz7y";
              };
              dontDisableStatic = true;
            });
          in naersk-lib.buildPackage rec {
            name = "flux-screensaver-windows";
            src = ./windows;
            release = true;
            gitAllRefs = true;

            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ SDL2_static ];

            shellHook = preBuild;
            preBuild = ''
              export NIX_LDFLAGS="$NIX_LDFLAGS -L ${SDL2_static}/lib"
            '';
          };

          flux-screensaver-windows = let
            inherit (pkgs.pkgsCross) mingwW64;
            SDL2_static = pkgs.pkgsCross.mingwW64.SDL2.overrideAttrs (old: rec {
              version = "2.0.22";
              name = "SDL2-static-${version}";
              src = builtins.fetchurl {
                url =
                  "https://www.libsdl.org/release/${old.pname}-${version}.tar.gz";
                sha256 =
                  "sha256:0bkzd5h7kn4xmd93hpbla4n2f82nb35s0xcs4p3kybl84wqvyz7y";
              };
              dontDisableStatic = true;
            });
          in naersk-lib.buildPackage rec {
            name = "flux-screensaver-windows";
            src = ./windows;
            release = true;
            singleStep = true;
            gitAllRefs = true;

            nativeBuildInputs = [ mingwW64.stdenv.cc ];
            buildInputs = [
              # Needed by windres
              mingwW64.stdenv.cc
              # Dig out windres from the depths of gcc
              mingwW64.stdenv.cc.bintools.bintools_bin
              mingwW64.windows.pthreads
              mingwW64.windows.mingw_w64_pthreads
              SDL2_static
              pkgs.ripgrep
            ];

            CARGO_BUILD_TARGET = "x86_64-pc-windows-gnu";
            CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER =
              with pkgs.pkgsCross.mingwW64.stdenv;
              "${cc}/bin/${cc.targetPrefix}gcc";

            shellHook = preBuild;

            # Hack around dependencies having build scripts when cross-compiling
            # https://github.com/nix-community/naersk/issues/181
            preBuild = ''
              export NIX_LDFLAGS="$NIX_LDFLAGS -L ${SDL2_static}/lib"
              export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C link-args=$(echo $NIX_LDFLAGS | rg  '(-L\s?\S+)\s?' --only-matching)"
              export NIX_LDFLAGS=
            '';

            # Change the extension to .scr (Windows screensaver)
            postInstall = ''
              mv $out/bin/${name}.exe "$out/bin/Flux.scr"
            '';
          };
        };
      });
}
