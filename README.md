<a href="https://flux.sandydoo.me/">
  <img width="100%" src="https://github.com/sandydoo/gif-storage/blob/main/flux/social-header-2022-02-03.gif" alt="Flux" />
</a>

# Screensavers for Flux

#### [You can now buy Flux as a Windows screensaver →][store]
Enjoy staring at it for hours as your computer idles and help support development. More platforms coming soon!

---

I’m working on creating native screensavers for [Flux][flux] — a fluid simulation inspired by the macOS Drift screensaver.

This repository contains:

- `flux-ffi` — a foreign function interface for the [Flux library][flux].
- Native screensavers for the following platforms:
  - [MacOS](#macos)
  - [Windows](#windows)
  - ~Linux~

## Build for platform

### macOS

Build with XCode.

```sh
cd macos
xcodebuild -project Flux.xcodeproj/ -scheme Flux build
````

XCode should automatically build the FFI crate. In case it doesn’t, here’s how to do it manually.

```sh
cd flux-ffi
export MACOSX_DEPLOYMENT_TARGET=10.10
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin
lipo target/aarch64-apple-darwin/release/libflux.a target/x86_64-apple-darwin/release/libflux.a -create -output libflux.a
```

### Windows

This repo is set up to cross-compile Windows binaries from Linux using Nix.

```sh
nix build ./#flux-screensaver-windows
```

There’s also a cross-compilation development shell.

```sh
nix develop ./#cross-windows
cd windows
cargo build --target x86_64-pc-windows-gnu --release
```

Native Windows builds are currently untested. You’ll need Rust and a static build of SDL2 linked at build time.


[flux]: https://github.com/sandydoo/flux
[store]: https://sandydoo.gumroad.com/l/flux
