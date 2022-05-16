{ pkgs, stdenv, ...}:

stdenv.mkDerivation {
  name = "flux-linux-screensaver-wrapper";

  nativeBuildInputs = with pkgs; [ libGL ];
};
