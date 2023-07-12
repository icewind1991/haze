{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-23.05";
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    rust-overlay.inputs.flake-utils.follows = "flake-utils";
    cross-naersk.url = "github:icewind1991/cross-naersk";
    cross-naersk.inputs.nixpkgs.follows = "nixpkgs";
    cross-naersk.inputs.naersk.follows = "naersk";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    naersk,
    rust-overlay,
    cross-naersk,
  }: let
    inherit (builtins) filter;
    inherit (nixpkgs.lib.strings) hasInfix;
    targets = [
      "x86_64-unknown-linux-gnu"
      "x86_64-unknown-linux-musl"
      "aarch64-unknown-linux-gnu"
      "aarch64-unknown-linux-musl"
    ];
    releaseTargets = filter (hasInfix "-musl") targets;
  in
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
          config.allowUnfree = true;
        };
        inherit (pkgs) lib callPackage;
        inherit (lib.sources) sourceByRegex;
        inherit (lib.attrsets) genAttrs;

        hostTarget = pkgs.hostPlatform.config;
        cross-naersk' = callPackage cross-naersk {inherit naersk;};

        src = sourceByRegex ./. ["Cargo.*" "(src)(/.*)?"];

        nearskOpt = {
          pname = "haze";
          root = src;
        };
        buildTarget = target: (cross-naersk'.buildPackage target) nearskOpt;
        hostNaersk = cross-naersk'.hostNaersk;
      in rec {
        # `nix build`
        packages =
          genAttrs targets buildTarget
          // rec {
            haze = packages.${hostTarget};
            check = hostNaersk.buildPackage (nearskOpt
              // {
                mode = "check";
              });
            test = hostNaersk.buildPackage (nearskOpt
              // {
                mode = "test";
              });
            clippy = hostNaersk.buildPackage (nearskOpt
              // {
                mode = "clippy";
              });
            default = haze;
          };

        devShells = {
          default = cross-naersk'.mkShell targets {
            nativeBuildInputs = with pkgs; [bacon cargo-edit cargo-outdated clippy];
          };
        };
      }
    )
    // {
      inherit targets releaseTargets;
      homeManagerModule = import ./hm-module.nix self.packages;
    };
}
