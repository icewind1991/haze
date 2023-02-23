use camino::Utf8PathBuf;
use directories_next::ProjectDirs;
use miette::{IntoDiagnostic, Report, Result, WrapErr};
use serde::Deserialize;
use std::convert::TryFrom;
use std::env::var;
use std::fs::read_to_string;
use std::net::IpAddr;

#[derive(Debug, Deserialize)]
#[serde(from = "RawHazeConfig")]
pub struct HazeConfig {
    pub sources_root: Utf8PathBuf,
    pub work_dir: Utf8PathBuf,
    pub auto_setup: HazeAutoSetupConfig,
    pub volume: Vec<HazeVolumeConfig>,
    pub blackfire: Option<HazeBlackfireConfig>,
    pub proxy: ProxyConfig,
}

#[derive(Debug, Deserialize)]
pub struct RawHazeConfig {
    pub sources_root: Utf8PathBuf,
    #[serde(default = "default_work_dir")]
    pub work_dir: Utf8PathBuf,
    #[serde(default)]
    pub auto_setup: HazeAutoSetupConfig,
    #[serde(default)]
    pub volume: Vec<HazeVolumeConfig>,
    #[serde(default)]
    pub blackfire: Option<HazeBlackfireConfig>,
    #[serde(default)]
    pub proxy: ProxyConfig,
}

impl From<RawHazeConfig> for HazeConfig {
    fn from(raw: RawHazeConfig) -> Self {
        fn normalize_path(path: Utf8PathBuf) -> Utf8PathBuf {
            if path.starts_with("~") {
                let home = var("HOME").expect("HOME not set");
                format!("{}{}", home, &path.as_str()[1..]).into()
            } else {
                path
            }
        }

        HazeConfig {
            sources_root: normalize_path(raw.sources_root),
            work_dir: normalize_path(raw.work_dir),
            auto_setup: raw.auto_setup,
            volume: raw.volume,
            blackfire: raw.blackfire,
            proxy: raw.proxy,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct HazeAutoSetupConfig {
    pub enabled: bool,
    #[serde(default = "default_auto_setup_username")]
    pub username: String,
    #[serde(default = "default_auto_setup_password")]
    pub password: String,
    #[serde(default)]
    pub post_setup: Vec<String>,
}

impl Default for HazeAutoSetupConfig {
    fn default() -> HazeAutoSetupConfig {
        HazeAutoSetupConfig {
            enabled: false,
            username: default_auto_setup_username(),
            password: default_auto_setup_password(),
            post_setup: Vec::default(),
        }
    }
}

fn default_work_dir() -> Utf8PathBuf {
    "/tmp/haze".into()
}

fn default_auto_setup_username() -> String {
    "admin".to_string()
}

fn default_auto_setup_password() -> String {
    "admin".to_string()
}

#[derive(Debug, Deserialize)]
pub struct HazeVolumeConfig {
    pub source: Utf8PathBuf,
    pub target: Utf8PathBuf,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub create: bool,
}

#[derive(Debug, Deserialize)]
#[serde(try_from = "RawHazeBlackfireConfig")]
pub struct HazeBlackfireConfig {
    pub server_id: String,
    pub server_token: String,
    pub client_id: String,
    pub client_token: String,
}

#[derive(Debug, Deserialize)]
pub struct RawHazeBlackfireConfig {
    #[serde(default)]
    pub server_id: Option<String>,
    #[serde(default)]
    pub server_id_path: Option<String>,
    #[serde(default)]
    pub server_token: Option<String>,
    #[serde(default)]
    pub server_token_path: Option<String>,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_id_path: Option<String>,
    #[serde(default)]
    pub client_token: Option<String>,
    #[serde(default)]
    pub client_token_path: Option<String>,
}

impl TryFrom<RawHazeBlackfireConfig> for HazeBlackfireConfig {
    type Error = String;

    fn try_from(value: RawHazeBlackfireConfig) -> std::result::Result<Self, Self::Error> {
        Ok(HazeBlackfireConfig {
            server_id: load_secret("server_id", value.server_id_path, value.server_id)?,
            server_token: load_secret("server_token", value.server_token_path, value.server_token)?,
            client_id: load_secret("client_id", value.client_id_path, value.client_id)?,
            client_token: load_secret("client_token", value.client_token_path, value.client_token)?,
        })
    }
}

fn load_secret(name: &str, path: Option<String>, raw: Option<String>) -> Result<String, String> {
    match (path, raw) {
        (None, Some(raw)) => Ok(raw),
        (Some(path), None) => {
            read_to_string(&path).map_err(|e| format!("failed to load {name} from {path}: {e}"))
        }
        (Some(_), Some(_)) => Err(format!("both {name} and {name}_path are specified")),
        (None, None) => Err(format!("neither {name} nor {name}_path are specified")),
    }
}

#[derive(Default, Deserialize, Debug)]
pub struct ProxyConfig {
    pub listen: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub https: bool,
}

impl ProxyConfig {
    /// Get a public address for a service, either with direct ip or through the proxy
    pub fn addr(&self, id: &str, ip: IpAddr) -> String {
        let clean_id = id.strip_prefix("haze-").unwrap_or(&id);
        match (&self.address, self.https) {
            (public, true) if !public.is_empty() => format!("https://{clean_id}.{public}"),
            (public, false) if !public.is_empty() => format!("http://{clean_id}.{public}"),
            _ => format!("http://{ip}"),
        }
    }

    pub fn addr_with_port(&self, id: &str, ip: IpAddr, port: u16) -> String {
        let clean_id = id.strip_prefix("haze-").unwrap_or(&id);
        match (&self.address, self.https) {
            (public, true) if !public.is_empty() => format!("https://{clean_id}.{public}"),
            (public, false) if !public.is_empty() => format!("http://{clean_id}.{public}"),
            _ => format!("http://{ip}:{port}"),
        }
    }
}

impl HazeConfig {
    pub fn load() -> Result<Self> {
        let dirs = ProjectDirs::from("nl", "icewind", "haze").unwrap();
        let file = dirs.config_dir().join("haze.toml");
        if !file.exists() {
            return Err(Report::msg(format!(
                "Config file not setup: {}",
                file.to_string_lossy()
            )));
        }
        let content = read_to_string(&file)
            .into_diagnostic()
            .wrap_err("Failed to read config file")?;
        toml::from_str(&content)
            .into_diagnostic()
            .wrap_err("Failed to parse config file")
    }
}
