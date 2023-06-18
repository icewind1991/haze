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

  outputs = { self, nixpkgs, flake-utils, naersk, rust-overlay, cross-naersk }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [ (import rust-overlay) ];
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

        execSufficForTarget = target: if lib.strings.hasInfix "windows" target then ".exe" else "";
        artifactForTarget = target: "palantir${execSufficForTarget target}";
        assetNameForTarget = target: "palantir-${builtins.replaceStrings ["-unknown" "-gnu" "-musl" "abihf" "-pc"] ["" "" "" "" ""] target}${execSufficForTarget target}";

        toolchain = (pkgs.rust-bin.stable.latest.default.override { inherit targets; });
        cross-naersk' = pkgs.callPackage cross-naersk {inherit naersk;};

        src = lib.sources.sourceByRegex (lib.cleanSource ./.) ["Cargo.*" "(src)(/.*)?"];

        nearskOpt = {
          pname = "haze";
          root = src;
        };
        buildTarget = target: (cross-naersk' target).buildPackage nearskOpt;
        hostNaersk = (cross-naersk' hostTarget);
      in
        rec {
          # `nix build`
          packages = lib.attrsets.genAttrs targets buildTarget // rec {
            haze = packages.${hostTarget};
            check = hostNaersk.buildPackage (nearskOpt // {
              mode = "check";
            });
            test = hostNaersk.buildPackage (nearskOpt // {
              mode = "test";
            });
            clippy = hostNaersk.buildPackage (nearskOpt // {
              mode = "clippy";
            });
            default = haze;
          };

          inherit targets;
          releaseMatrix = {
            include = builtins.map (target: {
              inherit target;
              artifact_name = artifactForTarget target;
              asset_name = assetNameForTarget target;
            }) releaseTargets;
          };

          devShells = {
            default = pkgs.mkShell {
              nativeBuildInputs = with pkgs; [ rustc cargo bacon cargo-edit cargo-outdated clippy ];
            };
          };
        }
    ) // {
      homeManagerModule = {
        config,
        lib,
        pkgs,
        ...
      }:
        with lib; let
          cfg = config.programs.haze;
          format = pkgs.formats.toml {};
          configFile = format.generate "haze.toml" ({
            sources_root = cfg.sourcesRoot;
            work_dir = cfg.workDir;
            auto_setup = {
              enabled = cfg.autoSetup.enable;
              post_setup = cfg.autoSetup.postSetup;
            };
            volume = map (volume: {
              inherit (volume) source target;
              read_only = volume.readOnly;
            }) cfg.volumes;
          } // (if (cfg.blackfire == null) then {} else {
            blackfire = {
              client_id_path = cfg.blackfire.clientIdPath;
              client_token_path = cfg.blackfire.clientTokenPath;
              server_id_path = cfg.blackfire.serverIdPath;
              server_token_path = cfg.blackfire.serverTokenPath;
            };
          }) // (if (cfg.proxy == null) then {} else {
            proxy = {
              inherit (cfg.proxy) listen https address;
            };
          }));
          pkg = self.packages.${pkgs.system}.default;
        in {
          options.programs.haze = {
            enable = mkEnableOption "haze";

            sourcesRoot = mkOption {
              type = types.str;
              example = "/srv/http/cloud";
              description = "Path containing the Nextcloud sources";
            };

            workDir = mkOption {
              type = types.str;
              default = "~/.cache/haze";
              description = "Directory to store instance data";
            };

            autoSetup = mkOption {
              type = types.submodule {
                options = {
                  enable = mkOption {
                    type = types.bool;
                    default = false;
                    description = "Enable auto setup";
                  };
                  postSetup = mkOption {
                    type = types.listOf types.str;
                    default = [];
                    description = "Commands to run post-setup";
                  };
                };
              };
            };

            blackfire = mkOption {
              default = null;
              type = types.nullOr (types.submodule {
                options = {
                  clientIdPath = mkOption {
                    type = types.str;
                    description = "Path containing the blackfire client_id";
                  };
                  clientTokenPath = mkOption {
                    type = types.str;
                    description = "Path containing the blackfire client_token";
                  };
                  serverIdPath = mkOption {
                    type = types.str;
                    description = "Path containing the blackfire server_id";
                  };
                  serverTokenPath = mkOption {
                    type = types.str;
                    description = "Path containing the blackfire server_token";
                  };
                };
              });
            };

            volumes = mkOption {
              default = [];
              type = types.listOf (types.submodule {
                options = {
                  source = mkOption {
                    type = types.str;
                    description = "Source path to mount into the instance";
                  };
                  target = mkOption {
                    type = types.str;
                    description = "Path to mount to volume at";
                  };
                  readOnly = mkOption {
                    type = types.bool;
                    default = false;
                    description = "Whether to mount the volume readonly";
                  };
                };
              });
            };

            proxy = mkOption {
              default = null;
              type = types.nullOr (types.submodule {
                options = {
                  listen = mkOption {
                    type = types.str;
                    description = "Listen address or socket path to listen to";
                  };
                  address = mkOption {
                    default = "";
                    type = types.str;
                    description = "Base address served by a reverse proxy to the haze proxy, instaces will be servered on subdomain of this address";
                  };
                  https = mkOption {
                    default = false;
                    type = types.bool;
                    description = "Whether the reverse proxy accepts https connections";
                  };
                };
              });
            };
          };

          config = mkIf cfg.enable {
            xdg.configFile."haze/haze.toml".source = configFile;
            home.packages = [pkg];

            systemd.user.services.haze = {
              Unit = {
                Description = "Haze reverse proxy";
              };

              Service = {
                ExecStart = "${pkg}/bin/haze proxy";
                Restart = "on-failure";
                RestartSec = 10;
              };
              Install = {
                WantedBy = optional (cfg.proxy != null && cfg.proxy.listen != "") "default.target";
              };
            };
          };
        };
    };
}
