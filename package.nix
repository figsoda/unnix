{
  lib,
  stdenv,
  rustPlatform,
  installShellFiles,
  pkg-config,
  withBubblewrap ? stdenv.isLinux,
  makeBinaryWrapper,
  xz,
  zstd,
  bubblewrap,
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
  ]
  ++ lib.optionals withBubblewrap [
    makeBinaryWrapper
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
    installManPage artifacts/*.1
    installShellCompletion artifacts/unnix.{bash,fish} --zsh artifacts/_unnix
  ''
  + lib.optionalString withBubblewrap ''
    wrapProgram $out/bin/unnix \
      --prefix PATH : ${lib.makeBinPath [ bubblewrap ]}
  '';
}
