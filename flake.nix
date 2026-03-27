{
  description = "Dev shell for Dioxus + ESP + WASM with nightly Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {
    nixpkgs,
    flake-utils,
    rust-overlay,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {inherit system overlays;};

        # Use the latest nightly toolchain from rust-overlay
        rustNightly = pkgs.rust-bin.nightly.latest.default.override {
          extensions = ["rust-src"];
          targets = ["wasm32-unknown-unknown"];
        };
      in {
        packages = {
          default = pkgs.hello;
        };

        devShells.default = pkgs.mkShell {
          name = "dev-shell-dioxus-esp-nightly";

          buildInputs = [
            rustNightly
            pkgs.espflash
            pkgs.esp-generate
            pkgs.ffmpeg
            pkgs.rustup
            pkgs.just
            pkgs.wasm-bindgen-cli_0_2_104
            pkgs.wasm-pack
            pkgs.binaryen
            pkgs.lld
            pkgs.llvm
            pkgs.dioxus-cli
          ];
        };
      }
    );
}
