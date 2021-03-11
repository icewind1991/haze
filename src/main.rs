use crate::config::HazeConfig;
use camino::Utf8Path;
use color_eyre::{eyre::WrapErr, Report, Result};
use std::fs::create_dir_all;

mod cloud;
mod config;
mod docker;

fn main() {
    let config = HazeConfig {
        sources_root: "/srv/http/owncloud".into(),
        work_dir: "/tmp/oc-docket".into(),
    };
}
