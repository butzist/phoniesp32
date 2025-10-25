{
  description = "Dev shell for Dioxus + ESP + WASM with nightly Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {inherit system overlays;};

        espToolchainVersion = "1.90.0";
        espCrosstoolToolchainVersion = "15.2.0_20250920";

        # Use the latest nightly toolchain from rust-overlay
        rustNightly = pkgs.rust-bin.nightly.latest.default.override {
          extensions = ["rust-src"];
          targets = ["wasm32-unknown-unknown"];
        };

        # Build dioxus-cli from GitHub
        dioxusCli = pkgs.rustPlatform.buildRustPackage rec {
          pname = "dioxus-cli";
          version = "0.7.0-rc.0";

          src = pkgs.fetchFromGitHub {
            owner = "DioxusLabs";
            repo = "dioxus";
            rev = "v${version}";
            sha256 = "025j9qxv5sbdsyjz0cfylqjj6znmyliihjzrlnryp1wbz51nn2cg";
          };

          nativeBuildInputs = [
            pkgs.cacert
            pkgs.pkg-config
          ];

          buildInputs = [pkgs.openssl];

          buildFeatures = [
            "no-downloads"
          ];

          cargoHash = "sha256-BZUIOfZ6ophsUQelpkqAaSUmWAsc/AeSAMwsx/nw1eA=";
          cargoBuildFlags = ["--package" "dioxus-cli"];
          doCheck = false;
        };
      in {
        packages.${system}.default = pkgs.hello;

        devShells.default = pkgs.mkShell {
          name = "dev-shell-dioxus-esp-nightly";

          buildInputs = [
            rustNightly
            pkgs.espup
            pkgs.espflash
            pkgs.esp-generate
            pkgs.rustup
            pkgs.just
            pkgs.wasm-bindgen-cli_0_2_104
            pkgs.binaryen
            pkgs.lld
            pkgs.llvm
            dioxusCli
          ];

          shellHook = ''
            echo "🦀 Entered dev shell with nightly Rust + Dioxus"
            flake_root=$(git rev-parse --show-toplevel 2>/dev/null || echo "$PWD")


            # Use rustup shims
            export PATH=${pkgs.rustup}/bin:$PATH
            export RUSTUP_HOME="$flake_root/.rustup"
            export ESP_EXPORT="$flake_root/.espup/export-esp.sh"
            mkdir -p "$flake_root/.espup"

            # Setup nix-installed toolchain in rustup
            rustup toolchain link nix-nightly ${rustNightly}
            rustup default nix-nightly

            # Install ESP toolchain if not present
            if [ -f $ESP_EXPORT ]; then
              echo "ESP toolchain already installed, skipping install."
            else
              espup install --export-file $ESP_EXPORT --targets esp32 \
                --toolchain-version ${espToolchainVersion} \
                --crosstool-toolchain-version ${espCrosstoolToolchainVersion}
            fi
            source $ESP_EXPORT

          '';
        };
      }
    );
}
