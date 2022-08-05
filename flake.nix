{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
  };

  outputs = { self, nixpkgs, flake-utils, naersk }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = nixpkgs.legacyPackages."${system}";
        naersk-lib = naersk.lib."${system}";
      in
        rec {
          # `nix build`
          packages.haze = naersk-lib.buildPackage {
            pname = "haze";
            root = ./.;
          };
          defaultPackage = packages.haze;
          defaultApp = packages.haze;

          # `nix develop`
          devShell = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [ rustc cargo bacon cargo-edit cargo-outdated ];
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
          }));
          pkg = self.defaultPackage.${pkgs.system};
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
          };

          config = mkIf cfg.enable {
            xdg.configFile."haze/haze.toml".source = configFile;
            home.packages = [pkg];
          };
        };
    };
}
