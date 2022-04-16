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
          flux-windows = naersk-lib.buildPackage rec {
            name = "flux-windows";
            version = "latest";
            src = ./windows;

            nativeBuildInputs = with pkgs.pkgsCross.mingwW64; [ stdenv.cc ];

            buildInputs = with pkgs.pkgsCross.mingwW64; [
              windows.mingw_w64_pthreads
              windows.pthreads
              pkgs.ripgrep
              SDL2
            ];

            CARGO_BUILD_TARGET = "x86_64-pc-windows-gnu";
            CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER =
              with pkgs.pkgsCross.mingwW64.stdenv;
              "${cc}/bin/${cc.targetPrefix}gcc";

            singleStep = true;

            preBuild = ''
              export SDL2_INCLUDE_PATH=${pkgs.pkgsCross.mingwW64.SDL2.dev}/include
              export NIX_LDFLAGS="$NIX_LDFLAGS -L ${pkgs.pkgsCross.mingwW64.SDL2}/bin"
              export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C link-args=$(echo $NIX_LDFLAGS | rg  '(-L.*)(\s|$)' --only-matching)"
              export NIX_LDFLAGS=
            '';

            # TODO: change binary extension to `scr`
            postInstall = ''
              cp ${pkgs.pkgsCross.mingwW64.SDL2}/bin/SDL2.dll $out/bin/SDL2.dll
            '';
          };
        };

        defaultPackage = packages.flux-windows;

        devShell = pkgs.mkShell {
          inputsFrom = [ packages.flux-windows ];
          packages = with pkgs; [ toolchain nix-fmt ripgrep ];
        };
      });
}
