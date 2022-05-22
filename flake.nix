{
  description = "Flux screensavers";

  inputs = {
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    naersk = {
      url = "github:nmattia/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, fenix, flake-utils, naersk, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        inherit (pkgs) lib stdenv;
        toolchain = with fenix.packages.${system};
          combine ([
            latest.rustc
            latest.cargo
            targets.x86_64-pc-windows-gnu.latest.rust-std
          ]);

        naersk-lib = naersk.lib.${system}.override {
          rustc = toolchain;
          cargo = toolchain;
        };
      in rec {
        packages = {
          flux-screensaver-windows = let
            SDL2_static = pkgs.pkgsCross.mingwW64.SDL2.overrideAttrs (old: rec {
              version = "2.0.22";
              name = "SDL2-static-${version}";
              src = builtins.fetchurl {
                url = "https://www.libsdl.org/release/${old.pname}-${version}.tar.gz";
                sha256 = "sha256:0bkzd5h7kn4xmd93hpbla4n2f82nb35s0xcs4p3kybl84wqvyz7y";
              };
              dontDisableStatic = true;
            });
          in naersk-lib.buildPackage rec {
            name = "flux-windows-screensaver";
            src = ./windows;

            nativeBuildInputs = with pkgs.pkgsCross.mingwW64; [ stdenv.cc ];

            buildInputs = with pkgs.pkgsCross.mingwW64; [
              # Dig out windres from the depths of gcc
              pkgs.pkgsCross.mingwW64.stdenv.cc.bintools.bintools_bin
              # Needed by windres
              pkgs.pkgsCross.mingwW64.stdenv.cc

              windows.mingw_w64_pthreads
              windows.pthreads
              SDL2_static
              pkgs.ripgrep
            ];

            CARGO_BUILD_TARGET = "x86_64-pc-windows-gnu";
            CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER =
              with pkgs.pkgsCross.mingwW64.stdenv;
              "${cc}/bin/${cc.targetPrefix}gcc";

            singleStep = true;

            # Hack around dependencies having build scripts when cross-compiling
            # https://github.com/nix-community/naersk/issues/181
            preBuild = ''
              export NIX_LDFLAGS="$NIX_LDFLAGS -L ${SDL2_static}/lib"
              export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C link-args=$(echo $NIX_LDFLAGS | rg  '(-L\s?\S+)\s?' --only-matching)"
              export NIX_LDFLAGS=
            '';

            # Change the extension to .scr (Windows screensaver)
            postInstall = ''
              mv $out/bin/${name}.exe $out/bin/${name}.scr
            '';
          };
        };

        defaultPackage = packages.flux-screensaver-windows;

        devShell = pkgs.mkShell {
          inputsFrom = [ packages.flux-screensaver-windows ];
          packages = with pkgs; [ toolchain nixfmt ripgrep ];
        };
      });
}
