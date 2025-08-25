{
  description = "the-editor nix flake";

  inputs = {
    nixpkgs.url     = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url       = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    fenix.url       = "github:nix-community/fenix";    
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      fenix,
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
        
        # Custom filter that includes Cargo files, Rust files, and assets.
        src = lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            (lib.hasSuffix ".rs" path) ||
            (lib.hasSuffix ".toml" path) ||
            (lib.hasSuffix ".ttf" path) ||
            (lib.hasSuffix ".otf" path) ||
            (lib.hasInfix "/assets/" path) ||
            (craneLib.filterCargoSources path type);
        };

        commonArgs = {
          inherit src;
          strictDeps = true;

          buildInputs = lib.optionals pkgs.stdenv.isLinux systemLibsLinux
            ++ lib.optionals pkgs.stdenv.isDarwin [
              pkgs.libiconv
          ];
          
          # NOTE: Env vars.
          # MISTER = "clayton";
        };

        # Build dependencies separately for better caching.
        cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
          pname = "the-editor-deps";
        });
        the-editor-unwrapped = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            pname = "the-editor";
          }
        );
        
        # Wrap the binary with runtime dependencies.
        # NOTE: So that `nix run` works.
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
          "the-editor " = the-editor-unwrapped;

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
              env.RUSTDOCFLAGS = "--deny warnings";
            }
          );

          "the-editor-fmt" = craneLib.cargoFmt {
            inherit src;
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
              cargoNextestPartitionsExtraArgs = "--no-tests=pass";
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
              ]))}:$LD_LIBRARY_PATH
            ''}
            
            # Use local target directory for incremental compilation.
            export CARGO_TARGET_DIR="target"
          '';
        };
      }
    );
}
