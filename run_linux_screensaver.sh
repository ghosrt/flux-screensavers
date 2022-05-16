#!/bin/sh

export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$(nix eval nixpkgs#libGL.outPath --raw)/lib
/home/sandydoo/Programming/flux-screensavers/linux/target/debug/flux-linux-screensaver
