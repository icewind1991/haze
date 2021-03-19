use crate::args::HazeArgs;
use crate::cloud::Cloud;
use crate::config::HazeConfig;
use crate::exec::container_logs;
use crate::service::Service;
use bollard::Docker;
use color_eyre::{eyre::WrapErr, Result};

mod args;
mod cloud;
mod config;
mod database;
mod exec;
mod image;
mod mapping;
mod php;
mod service;

#[tokio::main]
async fn main() -> Result<()> {
    let mut docker =
        Docker::connect_with_local_defaults().wrap_err("Failed to connect to docker")?;
    let config = HazeConfig::load().wrap_err("Failed to load config")?;

    let args = HazeArgs::parse(std::env::args())?;

    match args {
        HazeArgs::Clean => {
            let list = Cloud::list(&mut docker, None, &config).await?;
            for cloud in list {
                if let Err(e) = cloud.destroy(&mut docker).await {
                    eprintln!("Error while removing cloud: {:#}", e);
                }
            }
        }
        HazeArgs::List { filter } => {
            let list = Cloud::list(&mut docker, filter, &config).await?;
            for cloud in list {
                let mut services: Vec<_> = cloud.services.iter().map(Service::name).collect();
                services.push(cloud.db.name());
                let services = services.join(", ");
                match cloud.ip {
                    Some(ip) => println!(
                        "Cloud {}, {}, {}, running on http://{}",
                        cloud.id,
                        cloud.php.name(),
                        services,
                        ip
                    ),
                    None => println!(
                        "Cloud {}, {}, {}, not running",
                        cloud.id,
                        cloud.php.name(),
                        services
                    ),
                }
            }
        }
        HazeArgs::Start { options } => {
            let cloud = Cloud::create(&mut docker, options, &config).await?;
            println!("http://{}", cloud.ip.unwrap());
            if config.auto_setup.enabled {
                println!("Waiting for servers to start");
                cloud.wait_for_start(&mut docker).await?;
                println!(
                    "Installing with username {} and password {}",
                    config.auto_setup.username, config.auto_setup.password
                );
                cloud
                    .exec(
                        &mut docker,
                        vec![
                            "install",
                            &config.auto_setup.username,
                            &config.auto_setup.password,
                        ],
                        false,
                    )
                    .await?;
                cloud
                    .exec(
                        &mut docker,
                        vec![
                            "sed",
                            "-i",
                            &format!("s/0 => 'localhost'/'{}'/", cloud.ip.unwrap()),
                            "config/config.php",
                        ],
                        false,
                    )
                    .await?;
            }
        }
        HazeArgs::Stop { filter } => {
            let cloud = Cloud::get_by_filter(&mut docker, filter, &config).await?;
            cloud.destroy(&mut docker).await?;
        }
        HazeArgs::Logs {
            filter,
            count,
            service,
        } => {
            let cloud = Cloud::get_by_filter(&mut docker, filter, &config).await?;
            let container = if let Some(service) = service {
                service.container_name(&cloud.id)
            } else {
                cloud.id
            };
            let logs = container_logs(&docker, &container, count.unwrap_or(20)).await?;
            for log in logs {
                print!("{}", log);
            }
        }
        HazeArgs::Exec { filter, command } => {
            let cloud = Cloud::get_by_filter(&mut docker, filter, &config).await?;
            cloud
                .exec(
                    &mut docker,
                    if command.is_empty() {
                        vec!["bash".to_string()]
                    } else {
                        command
                    },
                    true,
                )
                .await?;
        }
        HazeArgs::Occ {
            filter,
            mut command,
        } => {
            let cloud = Cloud::get_by_filter(&mut docker, filter, &config).await?;
            command.insert(0, "occ".to_string());
            cloud.exec(&mut docker, command, true).await?;
        }
        HazeArgs::Db { filter } => {
            let cloud = Cloud::get_by_filter(&mut docker, filter, &config).await?;
            cloud.db.exec(&mut docker, &cloud.id).await?;
        }
        HazeArgs::Open { filter } => {
            let cloud = Cloud::get_by_filter(&mut docker, filter, &config).await?;
            match cloud.ip {
                Some(ip) => opener::open(format!("http://{}", ip))?,
                None => eprintln!("{} is not running", cloud.id),
            }
        }
        HazeArgs::Test { options, path } => {
            let cloud = Cloud::create(&mut docker, options, &config).await?;
            println!("Waiting for servers to start");
            cloud.wait_for_start(&mut docker).await?;
            println!("Installing");
            if let Err(e) = cloud
                .exec(
                    &mut docker,
                    vec![
                        "install",
                        &config.auto_setup.username,
                        &config.auto_setup.password,
                    ],
                    false,
                )
                .await
            {
                cloud.destroy(&mut docker).await?;
                return Err(e);
            }
            if let Some(app) = path
                .as_ref()
                .and_then(|path| path.strip_prefix("apps/"))
                .map(|path| &path[0..path.find('/').unwrap_or(path.len())])
            {
                if app.starts_with("files_") {
                    cloud.enable_app(&mut docker, "files_external").await?;
                }
                println!("Enabling {}", app);
                cloud.enable_app(&mut docker, app).await?;
            }
            cloud
                .exec(
                    &mut docker,
                    vec!["tests".to_string(), path.unwrap_or_default()],
                    false,
                )
                .await?;
            cloud.destroy(&mut docker).await?;
        }
    };

    Ok(())
}
