use crate::cloud::{parse, Cloud, CloudOptions};
use crate::config::HazeConfig;
use bollard::Docker;
use color_eyre::{eyre::WrapErr, Result};

mod cloud;
mod config;

#[tokio::main]
async fn main() -> Result<()> {
    let mut docker =
        Docker::connect_with_local_defaults().wrap_err("Failed to connect to docker")?;
    let config = HazeConfig {
        sources_root: "/srv/http/owncloud".into(),
        work_dir: "/tmp/haze".into(),
    };
    let options = CloudOptions::default();

    // let cloud = Cloud::create(&mut docker, options, &config).await?;
    // println!("{} running on http://{}", cloud.id, cloud.ip);

    let clouds = parse(&mut docker, &config).await?;
    dbg!(clouds);

    Ok(())
}
