{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    { self, nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        cargo_version =
          (builtins.fromTOML (builtins.readFile (self + "/Cargo.toml"))).package.version;
        version = "${cargo_version}-${self.shortRev or self.dirtyShortRev or "unknown"}";

        stasis = pkgs.rustPlatform.buildRustPackage {
          pname = "stasis";
          inherit version;
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          # Keep this: common + harmless, helps with any sys crates
          nativeBuildInputs = [ pkgs.pkg-config ];

          # New Stasis: Wayland + optional D-Bus integration (zbus)
          buildInputs = [
            pkgs.wayland
            pkgs.wayland-protocols
            pkgs.dbus
          ];

          # Optional; fine to keep for local perf, but remove if you want reproducible binaries
          RUSTFLAGS = "-C target-cpu=native";
        };
      in
      {
        packages.stasis = stasis;
        packages.default = stasis;

        formatter = pkgs.nixfmt;

        devShell = pkgs.mkShell {
          name = "stasis-devshell";
          buildInputs = [
            pkgs.rustc
            pkgs.cargo
            pkgs.pkg-config
            pkgs.git
            pkgs.wayland
            pkgs.wayland-protocols
            pkgs.dbus
          ];

          RUSTFLAGS = "-C target-cpu=native";

          shellHook = ''
            echo "Entering stasis dev shell — run: cargo build, cargo run, or nix build .#stasis"
          '';
        };
      }
    );
}
