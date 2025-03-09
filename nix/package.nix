{
  stdenv,
  rustPlatform,
  libsodium,
  pkg-config,
  lib,
}: let
  inherit (lib.sources) sourceByRegex;
  inherit (builtins) fromTOML readFile;
  src = sourceByRegex ../. ["Cargo.*" "(src)(/.*)?"];
  version = (fromTOML (readFile ../Cargo.toml)).package.version;
in
  rustPlatform.buildRustPackage rec {
    pname = "haze";

    inherit src version;

    cargoLock = {
      lockFile = ../Cargo.lock;
      outputHashes = {
        "hyper-reverse-proxy-0.5.2-dev" = "sha256-qO7eST4caHrqTXT7IgJ6aZxtCh/8QH5EsOxCRppH8d4=";
      };
    };
  }
