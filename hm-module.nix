{
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
      volume =
        map (volume: {
          inherit (volume) source target;
          read_only = volume.readOnly;
        })
        cfg.volumes;
      preset = cfg.presets;
    }
    // (
      if (cfg.blackfire == null)
      then {}
      else {
        blackfire = {
          client_id_path = cfg.blackfire.clientIdPath;
          client_token_path = cfg.blackfire.clientTokenPath;
          server_id_path = cfg.blackfire.serverIdPath;
          server_token_path = cfg.blackfire.serverTokenPath;
        };
      }
    )
    // (
      if (cfg.proxy == null)
      then {}
      else {
        proxy = {
          inherit (cfg.proxy) listen https address;
        };
      }
    ));
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

    presets = mkOption {
      default = [];
      type = types.listOf (types.submodule {
        options = {
          name = mkOption {
            type = types.str;
            description = "Name of the preset";
          };
          apps = mkOption {
            type = types.listOf types.str;
            description = "Apps to enable when the preset is enabled";
            default = [];
          };
          commands = mkOption {
            type = types.listOf types.str;
            description = "Commands to run post setup when the preset is enabled";
            default = [];
          };
          config = mkOption {
            type = types.submodule {
              freeformType = format.type;
            };
            description = "Configuration options to set before install";
            default = {};
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

    package = mkOption {
      type = types.package;
      defaultText = literalExpression "pkgs.haze";
      description = "package to use";
    };
  };

  config = mkIf cfg.enable {
    xdg.configFile."haze/haze.toml".source = configFile;
    home.packages = [cfg.package];

    systemd.user.services.haze = {
      Unit = {
        Description = "Haze reverse proxy";
      };

      Service = {
        ExecStart = "${cfg.package}/bin/haze proxy";
        Restart = "on-failure";
        RestartSec = 10;
      };
      Install = {
        WantedBy = optional (cfg.proxy != null && cfg.proxy.listen != "") "default.target";
      };
    };
  };
}
