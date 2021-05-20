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
    rnix-lsp
    rust-analyzer
    s6-networking
    systemd
    zlib
  ];
  shellHook = ''export CFG_DISABLE_CROSS_TESTS=1'';
}
