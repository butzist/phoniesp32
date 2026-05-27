{
  description = "Dev shell for Dioxus + ESP + WASM with nightly Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    flake-root.url = "github:srid/flake-root";
    rust-overlay.url = "github:oxalica/rust-overlay";
    jailed-agents.url = "github:andersonjoseph/jailed-agents";
  };

  outputs = inputs @ {
    nixpkgs,
    flake-parts,
    rust-overlay,
    jailed-agents,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux" "aarch64-darwin"];
      imports = [
        inputs.flake-root.flakeModule
      ];
      perSystem = {
        config,
        system,
        ...
      }: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {inherit system overlays;};

        jlib = jailed-agents.lib.${system};
        combinators = jlib.internals.jail.combinators;

        # Use the latest nightly toolchain from rust-overlay
        rustNightly = pkgs.rust-bin.nightly.latest.default.override {
          extensions = ["rust-src"];
          targets = ["wasm32-unknown-unknown"];
        };

        packages = [
          rustNightly
          pkgs.espflash
          pkgs.esp-generate
          pkgs.ffmpeg
          pkgs.cargo
          pkgs.just
          pkgs.wasm-bindgen-cli_0_2_114
          pkgs.wasm-pack
          pkgs.binaryen
          pkgs.lld
          pkgs.llvm
          pkgs.dioxus-cli
        ];
      in {
        devShells.default = pkgs.mkShell {
          name = "dev-shell-dioxus-esp-nightly";
          packages =
            packages
            ++ [
              (jlib.makeJailedOpencode {
                name = "opencode";
                extraPkgs = packages;
                extraReadwriteDirs = ["~/.cargo"];
                baseJailOptions = with combinators;
                  jlib.commonJailOptions
                  ++ [
                    (readwrite (noescape "\"$FLAKE_ROOT\""))
                  ];
              })
            ];
          inputsFrom = [config.flake-root.devShell]; # Provides $FLAKE_ROOT in dev shell
        };
      };
    };
}
