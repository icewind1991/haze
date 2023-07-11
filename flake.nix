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
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
          config.allowUnfree = true;
        };
        lib = pkgs.lib;

        hostTarget = pkgs.hostPlatform.config;
        targets = [
          hostTarget
          "x86_64-unknown-linux-musl"
        ];
        releaseTargets = lib.lists.remove hostTarget targets;

        execSufficForTarget = target:
          if lib.strings.hasInfix "windows" target
          then ".exe"
          else "";
        artifactForTarget = target: "palantir${execSufficForTarget target}";
        assetNameForTarget = target: "palantir-${builtins.replaceStrings ["-unknown" "-gnu" "-musl" "abihf" "-pc"] ["" "" "" "" ""] target}${execSufficForTarget target}";

        toolchain = pkgs.rust-bin.stable.latest.default.override {inherit targets;};
        cross-naersk' = pkgs.callPackage cross-naersk {inherit naersk;};

        src = lib.sources.sourceByRegex (lib.cleanSource ./.) ["Cargo.*" "(src)(/.*)?"];

        nearskOpt = {
          pname = "haze";
          root = src;
        };
        buildTarget = target: (cross-naersk'.buildPackage target) nearskOpt;
        hostNaersk = cross-naersk'.hostNaersk;
      in rec {
        # `nix build`
        packages =
          lib.attrsets.genAttrs targets buildTarget
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

        inherit targets;
        releaseMatrix = {
          include =
            builtins.map (target: {
              inherit target;
              artifact_name = artifactForTarget target;
              asset_name = assetNameForTarget target;
            })
            releaseTargets;
        };

        devShells = {
          default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [rustc cargo bacon cargo-edit cargo-outdated clippy];
          };
        };
      }
    )
    // {
      homeManagerModule = import ./hm-module.nix;
    };
}
