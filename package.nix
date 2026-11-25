{
  lib,
  stdenv,
  rustPlatform,
}:
rustPlatform.buildRustPackage {
  pname = "credential-store-pinentry";
  version = (lib.importTOML ./Cargo.toml).package.version;

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [./Cargo.toml ./Cargo.lock ./src ./tests];
  };

  cargoLock.lockFile = ./Cargo.lock;

  # Cross builds (Windows) cannot run the test binaries.
  doCheck = stdenv.buildPlatform.canExecute stdenv.hostPlatform;

  meta = {
    description = "pinentry that answers the OpenPGP card PIN from the macOS Keychain or Windows Credential Manager";
    homepage = "https://github.com/truelecter/credential-store-pinentry";
    license = lib.licenses.mit;
    mainProgram = "credential-store-pinentry";
    platforms = lib.platforms.darwin ++ lib.platforms.linux ++ lib.platforms.windows;
  };
}
