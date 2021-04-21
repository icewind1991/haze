use camino::Utf8PathBuf;
use color_eyre::{eyre::WrapErr, Report, Result};
use directories_next::ProjectDirs;
use serde::Deserialize;
use std::fs::read;

#[derive(Debug, Deserialize)]
pub struct HazeConfig {
    pub sources_root: Utf8PathBuf,
    #[serde(default = "default_work_dir")]
    pub work_dir: Utf8PathBuf,
    #[serde(default)]
    pub auto_setup: HazeAutoSetupConfig,
    #[serde(default)]
    pub volume: Vec<HazeVolumeConfig>,
}

#[derive(Debug, Deserialize)]
pub struct HazeAutoSetupConfig {
    pub enabled: bool,
    #[serde(default = "default_auto_setup_username")]
    pub username: String,
    #[serde(default = "default_auto_setup_password")]
    pub password: String,
}

impl Default for HazeAutoSetupConfig {
    fn default() -> HazeAutoSetupConfig {
        HazeAutoSetupConfig {
            enabled: false,
            username: default_auto_setup_username(),
            password: default_auto_setup_password(),
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
        let content = read(&file).wrap_err("Failed to read config file")?;
        toml::from_slice(&content).wrap_err("Failed to parse config file")
    }
}
