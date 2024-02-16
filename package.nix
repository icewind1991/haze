{
  stdenv,
  rustPlatform,
  libsodium,
  pkg-config,
  lib,
}: let
  inherit (lib.sources) sourceByRegex;
  src = sourceByRegex ./. ["Cargo.*" "(src)(/.*)?"];
in
  rustPlatform.buildRustPackage rec {
    pname = "haze";
    version = "0.1.0";

    inherit src;

    cargoLock = {
      lockFile = ./Cargo.lock;
      outputHashes = {
        "hyper-reverse-proxy-0.5.2-dev" = "sha256-8yBpYQZJaNhaecjR2GhQytRM4jgS0GaKzAxRXFmIf8k=";
      };
    };
  }
