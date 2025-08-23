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

        systemLibsLinux = [
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
        src      = craneLib.cleanCargoSource ./.;

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

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        the-editor     = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
          }
        );
      in
      {
        checks = {
          "the-editor " = the-editor ;

          "the-editor -clippy" = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );

          "the-editor -doc" = craneLib.cargoDoc (
            commonArgs
            // {
              inherit cargoArtifacts;
              env.RUSTDOCFLAGS = "--deny warnings";
            }
          );

          "the-editor -fmt" = craneLib.cargoFmt {
            inherit src;
          };

          "the-editor -toml-fmt" = craneLib.taploFmt {
            src = pkgs.lib.sources.sourceFilesBySuffices src [ ".toml" ];
          };

          "the-editor -nextest" = craneLib.cargoNextest (
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

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          # NOTE: Env again but for shell.
          # MISTER = "clayton again";

          packages = with pkgs; [
            sqlx-cli
            # NOTE: Extra packages for devShell.
          ] ++ lib.optionals pkgs.stdenv.isLinux systemLibsLinux;

          LD_LIBRARY_PATH = lib.makeLibraryPath systemLibsLinux;
        };
      }
    );
}
