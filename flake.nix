{
  description = "the-editor nix flake";

  inputs = {
    nixpkgs.url     = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url       = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    fenix.url       = "github:nix-community/fenix";
    ghostty.url     = "github:mitchellh/ghostty";
  };

  outputs =
    {
      nixpkgs,
      crane,
      flake-utils,
      fenix,
      ghostty,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        inherit (pkgs) lib;

        systemLibsLinux = with pkgs; [
          xorg.libX11
          xorg.libXcursor
          xorg.libXi
          xorg.libXrandr
          xorg.libxcb
          wayland
          libxkbcommon
          vulkan-loader
          vulkan-headers
          libGL
          # Linux specific OS dependencies.
        ];

        rustToolchain = fenix.packages.${system}.complete.withComponents [
          "rustc"
          "cargo"
          "clippy"
          "rustfmt"
          "rust-src"
          "rust-analyzer"
        ];

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # Get ghostty from the flake input
        ghosttyPkg = ghostty.packages.${system}.default;

        # Custom filter that includes Cargo files, Rust files, and assets.
        src = lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            (lib.hasSuffix ".rs" path) ||
            (lib.hasSuffix ".toml" path) ||
            (lib.hasSuffix ".ttf" path) ||
            (lib.hasSuffix ".otf" path) ||
            (lib.hasSuffix ".wgsl" path) ||
            (lib.hasSuffix ".scm" path) ||
            (lib.hasSuffix ".so" path) ||
            (lib.hasSuffix ".a" path) ||
            (lib.hasInfix "/assets/" path) ||
            (lib.hasInfix "/runtime/" path) ||
            (lib.hasInfix "/vendored/" path) ||
            (craneLib.filterCargoSources path type);
        };

        commonArgs = {
          inherit src;
          strictDeps = true;

          buildInputs = lib.optionals pkgs.stdenv.isLinux systemLibsLinux
            ++ lib.optionals pkgs.stdenv.isDarwin [
              pkgs.libiconv
            ]
            ++ [
              # Add ghostty library dependency for terminal crate linking
              ghosttyPkg
            ];

          nativeBuildInputs = [
            # No Zig needed - wrapper is pre-built and vendored
          ];

          # Set THE_EDITOR_DEFAULT_RUNTIME at compile time so tests can find runtime/ directory
          THE_EDITOR_DEFAULT_RUNTIME = "${src}/runtime";

          # Set library path for ghostty-vt linking
          LD_LIBRARY_PATH = "${ghosttyPkg}/lib";
        };

        # Build dependencies separately for better caching.
        cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
          src = src;
          cargoExtraArgs = "--workspace --locked";
        });
        the-editor-unwrapped = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            pname = "the-editor";
            cargoExtraArgs = "--features unicode-lines";
            # Disable tests in build (they run separately in nextest check)
            doCheck = false;

            # Zig wrapper and ghostty libraries are vendored (pre-built)
            # No compilation needed during Nix build
            preBuild = ''
              # Verify vendored libraries exist
              if [ ! -f the-terminal/vendored/linux-x86_64/libghostty_wrapper.a ]; then
                echo "ERROR: Vendored wrapper library not found"
                echo "Expected: the-terminal/vendored/linux-x86_64/libghostty_wrapper.a"
                exit 1
              fi
              if [ ! -f the-terminal/vendored/linux-x86_64/libghostty-vt.so ]; then
                echo "ERROR: Vendored ghostty library not found"
                echo "Expected: the-terminal/vendored/linux-x86_64/libghostty-vt.so"
                exit 1
              fi
            '';
          }
        );

        # Wrap binary with library paths for distribution
        the-editor = pkgs.runCommand "the-editor" {
          buildInputs = [ pkgs.makeWrapper ];
        } ''
          mkdir -p $out/bin
          makeWrapper ${the-editor-unwrapped}/bin/the-editor $out/bin/the-editor \
            --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath (systemLibsLinux ++ (with pkgs; [
              vulkan-loader
              libGL
            ]))}
        '';
      in
      {
        checks = {
          "the-editor " = the-editor;

          "the-editor-clippy" = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );

          "the-editor-doc" = craneLib.cargoDoc (
            commonArgs
            // {
              inherit cargoArtifacts;
              # env.RUSTDOCFLAGS = "--deny warnings";
            }
          );

          "the-editor-fmt" = craneLib.cargoFmt {
            inherit src;
            cargoFmtExtraArgs = "-- --unstable-features";
          };

          "the-editor-toml-fmt" = craneLib.taploFmt {
            src = pkgs.lib.sources.sourceFilesBySuffices src [ ".toml" ];
          };

          "the-editor-nextest" = craneLib.cargoNextest (
            commonArgs
            // {
              inherit cargoArtifacts;
              partitions = 1;
              partitionType = "count";
              cargoNextestPartitionsExtraArgs = "--no-tests=pass --features unicode-lines --workspace --exclude the-renderer";
            }
          );
        };

        packages = {
          default = the-editor ;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = the-editor ;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.zig_0_15
            pkgs.tree-sitter
          ] ++ lib.optionals pkgs.stdenv.isLinux systemLibsLinux
            ++ lib.optionals pkgs.stdenv.isDarwin [
              pkgs.libiconv
            ];

          shellHook = ''
            # NOTE: Set up proper library paths for Linux.
            ${lib.optionalString pkgs.stdenv.isLinux ''
              export LD_LIBRARY_PATH=${lib.makeLibraryPath (systemLibsLinux ++ (with pkgs; [
                vulkan-loader
                libGL
              ]))}:${ghosttyPkg}/lib:$LD_LIBRARY_PATH
            ''}

            ${lib.optionalString pkgs.stdenv.isDarwin ''
              export LD_LIBRARY_PATH=${ghosttyPkg}/lib:$LD_LIBRARY_PATH
            ''}

            # Use local target directory for incremental compilation.
            export CARGO_TARGET_DIR="target"

            # Configure Zig cache directory
            export ZIG_GLOBAL_CACHE_DIR="$PWD/.zig-cache"

            # Auto-build Zig wrapper if it doesn't exist
            if [ ! -f the-terminal/zig-out/lib/libghostty_wrapper.a ]; then
              echo "Building Zig wrapper library..."
              (cd the-terminal && zig build)
            fi
          '';
        };
      }
    );
}
