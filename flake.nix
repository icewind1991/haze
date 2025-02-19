{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-24.11";
    flakelight = {
      url = "github:nix-community/flakelight";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    mill-scale = {
      url = "github:icewind1991/mill-scale";
      inputs.flakelight.follows = "flakelight";
    };
  };
  outputs = {mill-scale, ...}:
    mill-scale ./. {
      crossTargets = [
        "x86_64-unknown-linux-gnu"
        "x86_64-unknown-linux-musl"
        "aarch64-unknown-linux-gnu"
        "aarch64-unknown-linux-musl"
      ];

      homeModules = {
        default = {
          pkgs,
          config,
          lib,
          ...
        }: {
          imports = [./nix/hm-module.nix];
          config = lib.mkIf config.programs.haze.enable {
            programs.haze.package = lib.mkDefault pkgs.haze;
          };
        };
      };
    };
}
