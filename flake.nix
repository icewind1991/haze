{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-23.11";
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
        overlays = [
          (import rust-overlay)
          (import ./overlay.nix)
        ];
        pkgs = import nixpkgs {
          inherit system overlays;
          config.allowUnfree = true;
        };
        inherit (pkgs) lib callPackage;
        inherit (lib.sources) sourceByRegex;
        inherit (lib.attrsets) genAttrs;

        hostTarget = pkgs.hostPlatform.config;
        cross-naersk' = callPackage cross-naersk {inherit naersk;};

        nearskOpt = {
          inherit (pkgs.haze) src pname;
        };
        buildTarget = target: (cross-naersk'.buildPackage target) nearskOpt;
        hostNaersk = cross-naersk'.hostNaersk;
        toolchain = pkgs.rust-bin.stable.latest.default;
      in rec {
        # `nix build`
        packages =
          genAttrs targets buildTarget
          // rec {
            inherit (pkgs) haze;
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
            nativeBuildInputs = with pkgs; [toolchain bacon cargo-edit cargo-outdated clippy];
          };
        };
      }
    )
    // {
      overlays.default = import ./overlay.nix;
      homeManagerModules.default = {
        pkgs,
        config,
        lib,
        ...
      }: {
        imports = [./hm-module.nix];
        config = lib.mkIf config.programs.haze.enable {
          programs.haze.package = lib.mkDefault pkgs.haze;
        };
      };
      inherit targets releaseTargets;
    };
}
