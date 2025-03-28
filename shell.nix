{ pkgs ? (import <nixpkgs> {}), ... }:

pkgs.mkShell {

  name = "unpack_methode";
  RUSTC_VERSION = "stable";
  shellHook = ''
    export PATH=$PATH:''${CARGO_HOME:-~/.cargo}/bin
    export PATH=$PATH:''${RUSTUP_HOME:-~/.rustup}/toolchains/$RUSTC_VERSION-x86_64-unknown-linux-gnu/bin/
    export RUSTUP_TOOLCHAIN=$RUSTC_VERSION-x86_64-unknown-linux-gnu
  '';

  packages = with pkgs; [
    rustup
  ];
}
