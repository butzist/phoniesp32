{
  description = "Dev shell for Rust audio project (rodio) with ALSA & udev";

  inputs = {
    # Use a reasonably recent nixpkgs; change the ref if you prefer another channel/revision
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = {
    self,
    nixpkgs,
  }: let
    systems = ["x86_64-linux"];
    mkPkgs = system: import nixpkgs {inherit system;};
  in {
    devShells = builtins.listToAttrs (map (
        system: let
          pkgs = mkPkgs system;

          buildInputs = with pkgs; [
            alsa-lib.dev
            alsa-lib
            udev.dev
            udev
          ];
        in {
          name = system;
          value = {
            default = pkgs.mkShell {
              # Developer-facing packages to build/run your Rust project
              inherit buildInputs;
              nativeBuildInputs = with pkgs; [pkg-config];

              LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;

              # A little shell hook so when you enter the shell you know what's available
              shellHook = ''
                echo "Entered dev shell for Rust audio project"
                echo "- ALSA libs: ${pkgs.alsa-lib}/lib"
                echo "- libudev: ${pkgs.udev}/lib"
                echo ""
                echo "To build: cargo build"
                echo "To run:   cargo run --release"
              '';
            };
          };
        }
      )
      systems);
  };
}
