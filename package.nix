{
  lib,
  rustPlatform,
  pkg-config,
  xz,
  zstd,
}:

rustPlatform.buildRustPackage {
  pname = "unnix";
  inherit ((lib.importTOML ./Cargo.toml).package) version;

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      ./Cargo.lock
      ./Cargo.toml
      ./src
    ];
  };

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs = [
    xz
    zstd
  ];

  env = {
    ZSTD_SYS_USE_PKG_CONFIG = true;
  };
}
