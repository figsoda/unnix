{
  inputs = {
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-analyzer-src.follows = "";
    };
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];

      imports = [ inputs.treefmt-nix.flakeModule ];

      perSystem =
        {
          config,
          inputs',
          lib,
          pkgs,
          ...
        }:
        let
          inherit (pkgs)
            callPackage
            mkShell
            pkgsStatic
            stdenv
            ;
        in
        {
          devShells.default = mkShell {
            env.UNNIX_LOG = "unnix=trace";
          };

          packages = {
            default = callPackage ./package.nix { };
          }
          // lib.optionalAttrs stdenv.isLinux {
            static = pkgsStatic.callPackage ./package.nix { };
          };

          checks =
            let
              devShells = lib.mapAttrs' (n: lib.nameValuePair "devShell-${n}") config.devShells;
              packages = lib.mapAttrs' (n: lib.nameValuePair "package-${n}") config.packages;
            in
            devShells // packages;

          treefmt = {
            programs = {
              # keep-sorted start block=yes
              actionlint.enable = true;
              deadnix.enable = true;
              keep-sorted = {
                priority = 1;
                enable = true;
              };
              nixfmt.enable = true;
              oxfmt.enable = true;
              rustfmt = {
                enable = true;
                package = inputs'.fenix.packages.latest.rustfmt;
              };
              shfmt.enable = true;
              statix.enable = true;
              taplo.enable = true;
              zizmor.enable = true;
              # keep-sorted end
            };
            settings.global.excludes = [
              "**/unnix.lock.json"
            ];
          };
        };
    };
}
