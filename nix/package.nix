{
  rustPlatform,
  pkg-config,
  lib,
}: let
  inherit (lib.sources) sourceByRegex;
  inherit (builtins) fromTOML readFile;
  src = sourceByRegex ../. ["Cargo.*" "(src|redis-certificates)(/.*)?"];
  version = (fromTOML (readFile ../Cargo.toml)).package.version;
in
  rustPlatform.buildRustPackage rec {
    pname = "haze";

    inherit src version;

    cargoLock = {
      lockFile = ../Cargo.lock;
      outputHashes = {
        "hyper-reverse-proxy-0.5.2-dev" = "sha256-+ebi4FVVkiOpf75e8K5oGkHJBYQjLNJhUPNj+78zd7Q=";
      };
    };
  }
