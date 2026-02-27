{
  lib,
  rustPlatform,
  installShellFiles,
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
      ./build.rs
      ./src
    ];
  };

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  nativeBuildInputs = [
    installShellFiles
    pkg-config
  ];

  buildInputs = [
    xz
    zstd
  ];

  env = {
    GENERATE_ARTIFACTS = "artifacts";
    ZSTD_SYS_USE_PKG_CONFIG = true;
  };

  postInstall = ''
    installManPage artifacts/unnix.1
    installShellCompletion artifacts/unnix.{bash,fish} --zsh artifacts/_unnix
  '';
}
