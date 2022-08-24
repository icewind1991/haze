use crate::args::{ExecService, HazeArgs};
use crate::cloud::{Cloud, CloudOptions};
use crate::config::HazeConfig;
use crate::exec::container_logs;
use crate::git::checkout_all;
use crate::network::clear_networks;
use crate::php::PhpVersion;
use crate::proxy::proxy;
use crate::service::Service;
use crate::service::ServiceTrait;
use bollard::Docker;
use miette::{IntoDiagnostic, Result, WrapErr};
use std::io::stdout;

mod args;
mod cloud;
mod config;
mod database;
mod exec;
mod git;
mod image;
mod mapping;
mod network;
mod php;
mod proxy;
mod service;

#[tokio::main]
async fn main() -> Result<()> {
    miette::set_panic_hook();
    tracing_subscriber::fmt::init();

    let mut docker = Docker::connect_with_local_defaults()
        .into_diagnostic()
        .wrap_err("Failed to connect to docker")?;
    let config = HazeConfig::load().wrap_err("Failed to load config")?;

    let args = HazeArgs::parse(std::env::args())?;

    match args {
        HazeArgs::Clean => {
            let list = Cloud::list(&mut docker, None, &config).await?;
            for cloud in list.into_iter().filter(|cloud| !cloud.pinned) {
                if let Err(e) = cloud.destroy(&mut docker).await {
                    eprintln!("Error while removing cloud: {:#}", e);
                }
            }
            clear_networks(&docker).await?;
        }
        HazeArgs::List { filter } => {
            let list = Cloud::list(&mut docker, filter, &config).await?;
            for cloud in list {
                let mut services: Vec<_> = cloud.services.iter().map(Service::name).collect();
                services.push(cloud.db.name());
                let services = services.join(", ");
                let pin = if cloud.pinned { "*" } else { "" };
                println!(
                    "Cloud {}{}, {}, {}, running on {}",
                    cloud.id,
                    pin,
                    cloud.php.name(),
                    services,
                    cloud.address
                );
            }
        }
        HazeArgs::Start { options } => {
            setup(&mut docker, options, &config).await?;
        }
        HazeArgs::Stop { filter } => {
            let cloud = Cloud::get_by_filter(&mut docker, filter, &config).await?;
            cloud.destroy(&mut docker).await?;
        }
        HazeArgs::Logs {
            filter,
            follow,
            count,
            service,
        } => {
            let cloud = Cloud::get_by_filter(&mut docker, filter, &config).await?;
            let container = if let Some(service) = service {
                service.container_name(&cloud.id)
            } else {
                cloud.id
            };
            container_logs(&docker, stdout(), &container, count.unwrap_or(20), follow).await?;
        }
        HazeArgs::Exec {
            filter,
            service,
            command,
        } => {
            let cloud = Cloud::get_by_filter(&mut docker, filter, &config).await?;
            match service {
                None => {
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
                Some(ExecService::Db) => {
                    cloud
                        .db
                        .exec_sh(
                            &mut docker,
                            &cloud.id,
                            if command.is_empty() {
                                vec!["bash".to_string()]
                            } else {
                                command
                            },
                            true,
                        )
                        .await?;
                }
            }
        }
        HazeArgs::Occ {
            filter,
            mut command,
        } => {
            let cloud = Cloud::get_by_filter(&mut docker, filter, &config).await?;
            command.insert(0, "occ".to_string());
            cloud.exec(&mut docker, command, true).await?;
        }
        HazeArgs::Db { filter, root } => {
            let cloud = Cloud::get_by_filter(&mut docker, filter, &config).await?;
            cloud.db.exec(&mut docker, &cloud.id, root).await?;
        }
        HazeArgs::Open { filter } => {
            let cloud = Cloud::get_by_filter(&mut docker, filter, &config).await?;
            match cloud.ip {
                Some(ip) => opener::open(format!("http://{}", ip)).into_diagnostic()?,
                None => eprintln!("{} is not running", cloud.id),
            }
        }
        HazeArgs::Test { options, mut args } => {
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
            if let Some(app) = args
                .first()
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
            args.insert(0, "tests".to_string());
            cloud.exec(&mut docker, args, false).await?;
            cloud.destroy(&mut docker).await?;
        }
        HazeArgs::Integration { options, mut args } => {
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
            args.insert(0, "integration".to_string());
            cloud.exec(&mut docker, args, false).await?;
            cloud.destroy(&mut docker).await?;
        }
        HazeArgs::Fmt { path } => {
            let cloud = Cloud::create(
                &mut docker,
                CloudOptions::default().with_php(PhpVersion::Php74),
                &config,
            )
            .await?;
            let mut out_buffer = Vec::<u8>::with_capacity(1024);
            println!("Waiting for servers to start");
            cloud.wait_for_start(&mut docker).await?;
            println!("Installing composer");
            if let Err(e) = cloud
                .exec_with_output(
                    &mut docker,
                    vec!["composer", "install"],
                    Some(&mut out_buffer),
                )
                .await
                .and_then(|c| c.to_result())
            {
                eprintln!("{}", String::from_utf8_lossy(&out_buffer));
                cloud.destroy(&mut docker).await?;
                return Err(e);
            }
            out_buffer.clear();
            println!("Formatting");
            if let Err(e) = cloud
                .exec(
                    &mut docker,
                    vec!["composer", "run", "cs:fix", path.as_str()],
                    false,
                )
                .await
            {
                cloud.destroy(&mut docker).await?;
                return Err(e);
            }
            println!("Cleanup");
            if let Err(e) = cloud
                .exec_with_output(
                    &mut docker,
                    vec!["git", "clean", "-fd", "lib/composer"],
                    Some(&mut out_buffer),
                )
                .await
                .and_then(|c| c.to_result())
            {
                eprintln!("{}", String::from_utf8_lossy(&out_buffer));
                cloud.destroy(&mut docker).await?;
                return Err(e);
            }
            if let Err(e) = cloud
                .exec_with_output(
                    &mut docker,
                    vec!["git", "checkout", "lib/composer"],
                    Some(&mut out_buffer),
                )
                .await
                .and_then(|c| c.to_result())
            {
                eprintln!("{}", String::from_utf8_lossy(&out_buffer));
                cloud.destroy(&mut docker).await?;
                return Err(e);
            }
            cloud.destroy(&mut docker).await?;
        }
        HazeArgs::Shell { command, options } => {
            let cloud = setup(&mut docker, options, &config).await?;
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
            cloud.destroy(&mut docker).await?;
        }
        HazeArgs::Pin { filter } => {
            let cloud = Cloud::get_by_filter(&mut docker, filter, &config).await?;
            cloud.pin(&mut docker).await?;
        }
        HazeArgs::Unpin { filter } => {
            let cloud = Cloud::get_by_filter(&mut docker, filter, &config).await?;
            cloud.unpin(&mut docker).await?;
        }
        HazeArgs::Proxy => {
            proxy(docker, config).await?;
        }
        HazeArgs::Checkout { branch } => {
            checkout_all(&config.sources_root, &branch)?;
        }
    };

    Ok(())
}

async fn setup(docker: &mut Docker, options: CloudOptions, config: &HazeConfig) -> Result<Cloud> {
    let cloud = Cloud::create(docker, options, &config).await?;
    println!("{}", cloud.address);
    let host = cloud.address.split_once("://").expect("no address?").1;
    if config.auto_setup.enabled {
        println!("Waiting for servers to start");
        cloud.wait_for_start(docker).await?;
        println!(
            "Installing with username {} and password {}",
            config.auto_setup.username, config.auto_setup.password
        );
        let ip_str = format!("{}", cloud.ip.unwrap());
        cloud
            .exec(
                docker,
                vec![
                    "install",
                    &config.auto_setup.username,
                    &config.auto_setup.password,
                ],
                false,
            )
            .await?;
        cloud
            .occ(
                docker,
                vec![
                    "config:system:set",
                    "overwrite.cli.url",
                    "--value",
                    &cloud.address,
                ],
                None,
            )
            .await?;
        cloud
            .occ(
                docker,
                vec!["config:system:set", "overwritehost", "--value", host],
                None,
            )
            .await?;

        let domains = [ip_str.as_str(), "cloud", &cloud.id, host];
        for (i, domain) in domains.iter().enumerate() {
            cloud
                .occ(
                    docker,
                    vec![
                        "config:system:set",
                        "trusted_domains",
                        &format!("{}", i),
                        "--value",
                        domain,
                    ],
                    None,
                )
                .await?;
        }

        for service in &cloud.services {
            for app in service.apps() {
                cloud
                    .exec(docker, vec!["occ", "app:enable", *app, "--force"], false)
                    .await?;
            }
        }
        for service in &cloud.services {
            for cmd in service.post_setup(&docker, &cloud.id).await? {
                cloud
                    .exec(docker, shell_words::split(&cmd).into_diagnostic()?, false)
                    .await?;
            }
        }
        for cmd in &config.auto_setup.post_setup {
            cloud
                .exec(docker, shell_words::split(&cmd).into_diagnostic()?, false)
                .await?;
        }
    }
    Ok(cloud)
}
