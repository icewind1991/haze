use crate::args::{HazeArgs, HazeCommand};
use crate::cloud::{list, Cloud, CloudOptions};
use crate::config::HazeConfig;
use bollard::Docker;
use color_eyre::{eyre::WrapErr, Report, Result};

mod args;
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

    let args = HazeArgs::parse(std::env::args())?;
    dbg!(&args);

    match args.command {
        HazeCommand::Clean => {
            let list = list(&mut docker, &config).await?;
            for cloud in list {
                if let Err(e) = cloud.destroy(&mut docker).await {
                    eprintln!("Error while removing cloud: {:#}", e);
                }
            }
        }
        HazeCommand::List => {
            let list = list(&mut docker, &config).await?;
            for cloud in list {
                if let Some(filter) = &args.id {
                    if !cloud.id.contains(filter.as_str()) {
                        continue;
                    }
                }
                println!(
                    "Cloud {}, {}, {}, running on http://{}",
                    cloud.id,
                    cloud.php.name(),
                    cloud.db.name(),
                    cloud.ip
                )
            }
        }
        HazeCommand::Start => {
            let (options, rest) = CloudOptions::parse(args.options)?;
            if let Some(next) = rest.first() {
                return Err(Report::msg(format!("Unknown option {}", next)));
            }
            let cloud = Cloud::create(&mut docker, options, &config).await?;
            println!("http://{}", cloud.ip);
        }
        _ => todo!(),
    };

    Ok(())
}
