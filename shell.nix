{ forCi ? false }:
let
  pkgs = import <nixpkgs> { };
in
with pkgs; mkShell {
  buildInputs = [
    cargo
    cmake
    libgit2
    openssl
    pkgconfig
    systemd
  ] ++ (pkgs.lib.optionals (!forCi) [
    rnix-lsp
    rust-analyzer
    s6-networking
    zlib
  ]);
  shellHook = ''export CFG_DISABLE_CROSS_TESTS=1'';
}
