use crate::args::{HazeArgs, HazeCommand};
use crate::cloud::{Cloud, CloudOptions};
use crate::config::HazeConfig;
use bollard::Docker;
use color_eyre::{eyre::WrapErr, Report, Result};

mod args;
mod cloud;
mod config;
mod database;
mod image;
mod php;
mod tty;

#[tokio::main]
async fn main() -> Result<()> {
    let mut docker =
        Docker::connect_with_local_defaults().wrap_err("Failed to connect to docker")?;
    let config = HazeConfig::load().wrap_err("Failed to load config")?;

    let args = HazeArgs::parse(std::env::args())?;

    match args.command {
        HazeCommand::Clean => {
            let list = Cloud::list(&mut docker, None, &config).await?;
            for cloud in list {
                if let Err(e) = cloud.destroy(&mut docker).await {
                    eprintln!("Error while removing cloud: {:#}", e);
                }
            }
        }
        HazeCommand::List => {
            let list = Cloud::list(&mut docker, args.options.first().cloned(), &config).await?;
            for cloud in list {
                if let Some(filter) = &args.id {
                    if !cloud.id.contains(filter.as_str()) {
                        continue;
                    }
                }
                match cloud.ip {
                    Some(ip) => println!(
                        "Cloud {}, {}, {}, running on http://{}",
                        cloud.id,
                        cloud.php.name(),
                        cloud.db.name(),
                        ip
                    ),
                    None => println!(
                        "Cloud {}, {}, {}, not running",
                        cloud.id,
                        cloud.php.name(),
                        cloud.db.name()
                    ),
                }
            }
        }
        HazeCommand::Start => {
            let (options, rest) = CloudOptions::parse(args.options)?;
            if let Some(next) = rest.first() {
                return Err(Report::msg(format!("Unknown option {}", next)));
            }
            let cloud = Cloud::create(&mut docker, options, &config).await?;
            println!("http://{}", cloud.ip.unwrap());
        }
        HazeCommand::Stop => {
            let cloud = Cloud::get_by_filter(&mut docker, args.id, &config).await?;
            cloud.destroy(&mut docker).await?;
        }
        HazeCommand::Logs => {
            let cloud = Cloud::get_by_filter(&mut docker, args.id, &config).await?;
            let logs = cloud.logs(&mut docker).await?;
            for log in logs {
                print!("{}", log);
            }
        }
        HazeCommand::Exec => {
            let cloud = Cloud::get_by_filter(&mut docker, args.id, &config).await?;
            cloud
                .exec(
                    &mut docker,
                    if args.options.is_empty() {
                        vec!["bash".to_string()]
                    } else {
                        args.options
                    },
                )
                .await?;
        }
        HazeCommand::Occ => {
            let cloud = Cloud::get_by_filter(&mut docker, args.id, &config).await?;
            let mut options = args.options;
            options.insert(0, "occ".to_string());
            cloud.exec(&mut docker, options).await?;
        }
        HazeCommand::Db => {
            let cloud = Cloud::get_by_filter(&mut docker, args.id, &config).await?;
            cloud.db.exec(&mut docker, &cloud.id).await?;
        }
        HazeCommand::Open => {
            let cloud = Cloud::get_by_filter(&mut docker, args.id, &config).await?;
            match cloud.ip {
                Some(ip) => opener::open(format!("http://{}", ip))?,
                None => eprintln!("{} is not running", cloud.id),
            }
        }
        HazeCommand::Test => {
            todo!();
        }
    };

    Ok(())
}
