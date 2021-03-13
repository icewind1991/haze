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
}

fn default_work_dir() -> Utf8PathBuf {
    "/tmp/haze".into()
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
