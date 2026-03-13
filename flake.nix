{
  description = "lol-autoq – League of Legends auto-queue CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane = {
      url = "github:ipetkov/crane";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" ];
        };

        # crane library pinned to our chosen Rust toolchain
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # Arguments shared between the deps-only and final build
        commonArgs = {
          src = craneLib.cleanCargoSource ./.;
          strictDeps = true;
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [ pkgs.openssl ];
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
        };

        # Build only vendor crates.  This derivation is cached by Nix as long
        # as Cargo.lock is unchanged, so all crates survive between rebuilds.
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Build the actual binary, re-using the cached crate artifacts above.
        lol-autoq = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          pname = "lol-autoq";
          version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
        });
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain

            # build essentials
            pkgs.pkg-config
            pkgs.openssl

            # dev tools
            pkgs.cargo-watch
            pkgs.cargo-edit
            pkgs.cargo-nextest

            # UI toolchain
            pkgs.dioxus-cli      # `dx` — build/serve Dioxus desktop apps
            pkgs.tailwindcss     # Tailwind CSS standalone CLI
            pkgs.bun             # fast JS runtime/package manager (runs Tailwind)
          ];

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          RUST_LOG = "info";

          # rustls uses the webpki-roots crate for its CA bundle, so no
          # system SSL library is needed at runtime.  pkg-config + openssl
          # are kept only in case any transitive dep still needs them at
          # build time.
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
        };

        packages.default = lol-autoq;

        apps = {
          # nix run              — run the CLI (default)
          default = {
            type = "app";
            program = "${lol-autoq}/bin/lol-autoq";
          };

          # nix run .#dev        — start Tailwind watcher + dx serve together
          dev = {
            type = "app";
            program = "${pkgs.writeShellApplication {
              name = "lol-autoq-dev";
              runtimeInputs = [ pkgs.tailwindcss pkgs.dioxus-cli ];
              text = ''
                mkdir -p assets
                trap 'kill 0' EXIT
                tailwindcss -i ./input.css -o ./assets/tailwind.css --watch &
                dx serve --platform desktop
              '';
            }}/bin/lol-autoq-dev";
          };

          # nix run .#build      — minify CSS and build release binary
          build = {
            type = "app";
            program = "${pkgs.writeShellApplication {
              name = "lol-autoq-build";
              runtimeInputs = [ pkgs.tailwindcss pkgs.dioxus-cli ];
              text = ''
                mkdir -p assets
                tailwindcss -i ./input.css -o ./assets/tailwind.css --minify
                dx build --platform desktop --release
              '';
            }}/bin/lol-autoq-build";
          };

          # nix run .#ui         — launch the desktop UI directly
          ui = {
            type = "app";
            program = "${pkgs.writeShellApplication {
              name = "lol-autoq-ui";
              runtimeInputs = [ lol-autoq ];
              text = ''
                lol-autoq ui
              '';
            }}/bin/lol-autoq-ui";
          };

          # nix run .#css        — one-shot CSS generation
          css = {
            type = "app";
            program = "${pkgs.writeShellApplication {
              name = "lol-autoq-css";
              runtimeInputs = [ pkgs.tailwindcss ];
              text = ''
                mkdir -p assets
                tailwindcss -i ./input.css -o ./assets/tailwind.css
              '';
            }}/bin/lol-autoq-css";
          };
        };
      }
    );
}
