{
  inputs = {
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];

      perSystem =
        {
          config,
          lib,
          pkgs,
          ...
        }:
        let
          inherit (pkgs)
            callPackage
            mkShell
            pkgsStatic
            ;
        in
        {
          devShells.default = mkShell {
            env.UNNIX_LOG = "unnix=trace";
          };

          packages = {
            default = callPackage ./package.nix { };
            static = pkgsStatic.callPackage ./package.nix { };
          };

          checks =
            let
              devShells = lib.mapAttrs' (n: lib.nameValuePair "devShell-${n}") config.devShells;
              packages = lib.mapAttrs' (n: lib.nameValuePair "package-${n}") config.packages;
            in
            devShells // packages;
        };
    };
}
